use rand::Rng;
use teloxide::prelude::*;

const HELP: &str = r#"
#歡迎光臨洗手室
/help - 檢視說明
/roll - 擲骰子
/dinner [options] - 晚餐吃什麼
    e.g. /dinner 八方雲集 Sukiya 臺鐵便當 元氣
"#;

fn dinner(options: String) -> String {
    let arr: Vec<&str> = options.split_whitespace().collect();
    let mut rng = rand::thread_rng();
    let index = rng.gen_range(0..arr.len());
    return arr[index].to_string();
}

async fn message_parser(msg: &Message, bot: &Bot) {
    let full_text = msg.text().unwrap();
    let command = full_text.split_whitespace().next().unwrap();
    match command {
        "/help" => {
            let _ = bot.send_message(msg.chat.id, HELP).await;
        }
        "/roll" => {
            let _ = bot.send_dice(msg.chat.id).await;
        }
        "/dinner" => {
            let options = full_text.splitn(2, ' ').nth(1).unwrap();
            let result = dinner(options.to_string());
            let _ = bot.send_message(msg.chat.id, result).await;
        }
        _ => {}
    }
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Starting throw dice bot...");

    let bot = Bot::from_env();

    teloxide::repl(bot, |bot: Bot, msg: Message| async move {
        message_parser(&msg, &bot).await;
        Ok(())
    })
    .await;
}
