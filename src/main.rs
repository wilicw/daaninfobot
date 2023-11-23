use grammers_client::{Client, Config, SignInError};
use grammers_session::Session;
use rand::Rng;
use std::env;
use std::io::{self, BufRead as _, Write as _};
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

async fn resolve_user(username: String) -> Result<String> {
    let api_id = env!("TG_ID").parse().expect("TG_ID invalid");
    let api_hash = env!("TG_HASH").to_string();

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

    if let Some(chat) = client.resolve_username(&username).await? {
        return Ok(chat.id().to_string());
    }

    Ok("".to_string())
}

const HELP: &str = r#"
#歡迎光臨洗手室
/help - 檢視說明
/roll - 擲骰子
/title user string  - 變更使用者標籤
/dinner options... - 晚餐吃什麼
    e.g. /dinner 八方雲集 Sukiya 臺鐵便當 元氣
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

fn dinner(options: String) -> String {
    let arr: Vec<&str> = options.split_whitespace().collect();
    let mut rng = rand::thread_rng();
    let index = rng.gen_range(0..arr.len());
    return arr[index].to_string();
}

async fn message_parser(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    let full_text = msg.text().unwrap();
    dbg!(full_text);
    match cmd {
        Command::Start | Command::Help => bot.send_message(msg.chat.id, HELP).await?,
        Command::Roll => {
            if rand::thread_rng().gen_range(0..=2) == 0 {
                bot.send_animation(msg.chat.id, InputFile::file("./rickroll-roll.gif"))
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
                match &entity.kind {
                    MessageEntityKind::Mention => {
                        username = full_text.get(offset + 1..offset + length).unwrap();
                        user_id = UserId(
                            resolve_user(username.to_string())
                                .await
                                .unwrap()
                                .parse()
                                .unwrap(),
                        );
                        title = full_text.get(offset + length + 0..).unwrap();
                        break;
                    }
                    MessageEntityKind::TextMention { user } => {
                        username = full_text.get(offset + 0..offset + length).unwrap();
                        user_id = user.id;
                        title = full_text.get(offset + length + 1..).unwrap();
                        break;
                    }
                    _ => {}
                }
            }

            if title.is_empty() | (user_id == UserId(0)) | username.is_empty() {
                bot.send_message(
                    msg.chat.id,
                    "請輸入選項 e.g. /title user string".to_string(),
                )
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
                        .await?
                } else {
                    bot.send_message(
                        msg.chat.id,
                        format!("{} 的標籤已變更為 {}", username, title),
                    )
                    .await?
                }
            }
        }
        Command::Dinner => {
            let result: String;
            if full_text.split_whitespace().count() < 2 {
                result = "請輸入選項 e.g. /dinner 八方雲集 Sukiya 臺鐵便當 元氣".to_string();
            } else {
                result = dinner(full_text.splitn(2, ' ').nth(1).unwrap().to_string());
            }
            bot.send_message(msg.chat.id, result).await?
        }
    };

    Ok(())
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    let bot = Bot::from_env();
    Command::repl(bot, message_parser).await;
}
