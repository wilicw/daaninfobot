use dotenv::dotenv;
use grammers_client::{Client, Config, SignInError};
use grammers_session::Session;
use rand::Rng;
use std::env;
use std::io::{self, BufRead as _, Write as _};
use std::sync::Arc;
use teloxide::{
    prelude::*,
    types::{InputFile, MessageEntityKind},
    utils::command::BotCommands,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

const SESSION_FILE: &str = "echo.session";

fn prompt(message: &str) -> Result<String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout.write_all(message.as_bytes())?;
    stdout.flush()?;

    let stdin = io::stdin();
    let mut stdin = stdin.lock();

    let mut line = String::new();
    stdin.read_line(&mut line)?;
    Ok(line)
}

async fn init_tg_client() -> Result<Client> {
    let api_id = env::var("TG_ID").unwrap().parse().expect("TG_ID invalid");
    let api_hash = &env::var("TG_HASH").unwrap();

    println!("Connecting to Telegram...");
    let client = Client::connect(Config {
        session: Session::load_file_or_create(SESSION_FILE)?,
        api_id,
        api_hash: api_hash.clone(),
        params: Default::default(),
    })
    .await?;
    println!("Connected!");

    // If we can't save the session, sign out once we're done.
    let mut sign_out = false;

    if !client.is_authorized().await? {
        println!("Signing in...");
        let phone = prompt("Enter your phone number (international format): ")?;
        let token = client.request_login_code(&phone).await?;
        let code = prompt("Enter the code you received: ")?;
        let signed_in = client.sign_in(&token, &code).await;
        match signed_in {
            Err(SignInError::PasswordRequired(password_token)) => {
                // Note: this `prompt` method will echo the password in the console.
                //       Real code might want to use a better way to handle this.
                let hint = password_token.hint().unwrap_or("None");
                let prompt_message = format!("Enter the password (hint {}): ", &hint);
                let password = prompt(prompt_message.as_str())?;

                client
                    .check_password(password_token, password.trim())
                    .await?;
            }
            Ok(_) => (),
            Err(e) => panic!("{}", e),
        };
        println!("Signed in!");
        match client.session().save_to_file(SESSION_FILE) {
            Ok(_) => {}
            Err(e) => {
                println!(
                    "NOTE: failed to save the session, will sign out when done: {}",
                    e
                );
                sign_out = true;
            }
        }
    }

    if sign_out {
        // TODO revisit examples and get rid of "handle references" (also, this panics)
        drop(client.sign_out_disconnect().await);
    }

    Ok(client)
}

const HELP: &str = r#"
\#歡迎光臨洗手室
/help \- 檢視說明
/roll \- 擲骰子
/title *@user* *string*  \- 變更使用者標籤
/dinner *options\.\.\.* \- 晚餐吃什麼
    e\.g\. `/dinner 八方雲集 Sukiya 臺鐵便當 元氣`
"#;

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
enum Command {
    Start,
    Help,
    Roll,
    Title,
    Dinner,
}

async fn message_parser(
    bot: Bot,
    msg: Message,
    cmd: Command,
    client: Arc<Client>,
) -> ResponseResult<()> {
    let full_text = msg.text().unwrap();
    match cmd {
        Command::Start | Command::Help => {
            bot.send_message(msg.chat.id, HELP)
                .reply_to_message_id(msg.id)
                .disable_notification(true)
                .parse_mode(teloxide::types::ParseMode::MarkdownV2)
                .await?
        }
        Command::Roll => {
            if rand::thread_rng().gen_range(0..=2) == 0 {
                bot.send_animation(msg.chat.id, InputFile::file("./rickroll-roll.gif"))
                    .reply_to_message_id(msg.id)
                    .disable_notification(true)
                    .await?
            } else {
                bot.send_dice(msg.chat.id).await?
            }
        }
        Command::Title => {
            let mut title: &str = "";
            let mut user_id: UserId = UserId(0);
            let mut username: &str = "";
            for entity in msg.entities().unwrap() {
                let offset = entity.offset;
                let length = entity.length;
                dbg!(&msg);
                match &entity.kind {
                    MessageEntityKind::Mention => {
                        username = full_text.get(offset + 1..offset + length).unwrap();
                        if let Ok(Some(chat)) = client.resolve_username(&username).await {
                            user_id = UserId(chat.id().to_string().parse().unwrap());
                            title = full_text.get(offset + length..).unwrap();
                        }
                        break;
                    }
                    MessageEntityKind::TextMention { user } => {
                        username = full_text.get(offset + 0..offset + length).unwrap();
                        user_id = user.id;
                        title = full_text.get(offset + length..).unwrap();
                        break;
                    }
                    _ => {}
                }
            }

            if title.is_empty() | (user_id == UserId(0)) | username.is_empty() {
                bot.send_message(
                    msg.chat.id,
                    "請輸入選項 e.g. /title @user string".to_string(),
                )
                .reply_to_message_id(msg.id)
                .disable_notification(true)
                .await?
            } else {
                let steps = async {
                    bot.promote_chat_member(msg.chat.id, user_id)
                        .can_change_info(false)
                        .can_delete_messages(false)
                        .can_invite_users(true)
                        .can_restrict_members(false)
                        .can_pin_messages(true)
                        .can_promote_members(false)
                        .await?;
                    bot.set_chat_administrator_custom_title(msg.chat.id, user_id, title)
                        .await
                };

                if let Err(_err) = steps.await {
                    bot.send_message(msg.chat.id, format!("{} 的標籤變更失敗", username))
                        .reply_to_message_id(msg.id)
                        .disable_notification(true)
                        .await?
                } else {
                    bot.send_message(msg.chat.id, format!("{} 的標籤已變更為{}", username, title))
                        .reply_to_message_id(msg.id)
                        .disable_notification(true)
                        .await?
                }
            }
        }
        Command::Dinner => {
            let result: String;
            if full_text.split_whitespace().count() < 2 {
                result = "請輸入選項 e.g. /dinner 八方雲集 Sukiya 臺鐵便當 元氣".to_string();
            } else {
                let arr: Vec<&str> = full_text
                    .splitn(2, ' ')
                    .nth(1)
                    .unwrap()
                    .split_whitespace()
                    .collect();
                let mut rng = rand::thread_rng();
                let index = rng.gen_range(0..arr.len());
                result = arr[index].to_string();
            }
            bot.send_message(msg.chat.id, result)
                .reply_to_message_id(msg.id)
                .disable_notification(true)
                .await?
        }
    };

    Ok(())
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    pretty_env_logger::init();
    let client = Arc::new(init_tg_client().await.unwrap());

    let bot = Bot::from_env();
    let handler =
        Update::filter_message().branch(dptree::entry().filter_command::<Command>().endpoint(
            move |msg: Message, bot: Bot, cmd: Command| {
                let _client = Arc::clone(&client);
                async move { message_parser(bot, msg, cmd, _client).await }
            },
        ));
    Dispatcher::builder(bot, handler) // Command::repl(bot, message_parser).await;
        .enable_ctrlc_handler()
        .build()
        .dispatch()
        .await;
}
