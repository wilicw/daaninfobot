use rand::Rng;
use teloxide::{
    prelude::*,
    types::{InputFile, MessageEntityKind},
    utils::command::BotCommands,
};

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
                        username = full_text.get(offset + 0..offset + length).unwrap();
                        user_id = UserId(1060366902);
                        title = full_text.get(offset + length + 1..).unwrap();
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
                bot.promote_chat_member(msg.chat.id, user_id)
                    .can_change_info(false)
                    .can_delete_messages(false)
                    .can_invite_users(true)
                    .can_restrict_members(false)
                    .can_pin_messages(true)
                    .can_promote_members(false)
                    .await?;
                bot.set_chat_administrator_custom_title(msg.chat.id, user_id, title)
                    .await?;
                bot.send_message(
                    msg.chat.id,
                    format!("{} 的標籤已變更為 {}", username, title),
                )
                .await?
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
    log::info!("Starting throw dice bot...");

    let bot = Bot::from_env();
    Command::repl(bot, message_parser).await;
}
