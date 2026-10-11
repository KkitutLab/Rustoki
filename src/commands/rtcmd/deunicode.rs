use serenity::all::Message;
use serenity::prelude::Context;

pub async fn handle_deunicode(ctx: &Context, msg: &Message, rest: &str) {
    let arg = rest.trim();
    if arg.is_empty() {
        let usage = "`deunicode [--upcase/-u] [--comma/-c] [--quote/-q] <string>`";
        if let Err(e) = msg.channel_id.say(&ctx.http, usage).await {
            eprintln!("[ERROR] <! say(): {:?}", e);
        }
        return;
    }

    let mut upcase = false;
    let mut comma = false;
    let mut quote = false;
    let mut text = String::new();

    for part in arg.split_whitespace() {
        match part {
            "--upcase" | "-u" => upcase = true,
            "--comma" | "-c" => comma = true,
            "--quote" | "-q" => quote = true,
            _ => {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(part);
            }
        }
    }

    let mut result = deunicode::deunicode(&text);

    if upcase {
        result = result.to_uppercase();
    }
    if quote {
        result = result
            .split_whitespace()
            .map(|w| format!("\"{}\"", w))
            .collect::<Vec<_>>()
            .join(" ");
    }
    if comma {
        result = result.split_whitespace().collect::<Vec<_>>().join(", ");
    }

    if let Err(e) = msg.channel_id.say(&ctx.http, result).await {
        eprintln!("[ERROR] <! say(): {:?}", e);
    }
}
