use once_cell::sync::Lazy;
use serenity::model::id::{ChannelId, GuildId, UserId};

pub static BOT_USERID: UserId = UserId::new(1286264098987835445);
pub static GUILD_ID: GuildId = GuildId::new(1027978441573793853);
pub static KKITUT_USERID: UserId = UserId::new(700176751422341201);
pub static KAGO_USERID: UserId = UserId::new(1561197947775885392);

pub static CHANNELID_LOG_SECRET: Lazy<ChannelId> =
    Lazy::new(|| ChannelId::new(1208334066873798726));
pub static CHANNELID_PIN: Lazy<ChannelId> = Lazy::new(|| ChannelId::new(1399760051291426847));

pub static DISCORD_ID_ADMINS: &[u64] = &[
    KKITUT_USERID.get(),
    KAGO_USERID.get(),
    1202514875797213184,
    507523158757212160,
    412180095827181570,
    682792713485418497,
];

pub static RTCMD_HELP_MSG: &str = "\
```
Rustoki Shell v0.1.0 -- made by Kkitut
A minimal shell-like interface for Debug or Admin purposes.

USAGE:
    <! <command> [options] [args]
    (Type `<! <command>` without arguments to see its usage.)

AVAILABLE COMMANDS:
    deunicode - Convert Unicode strings to ASCII-friendly output - https://github.com/kornelski/deunicode
    embed - Send the embed to the desired channel
    memory - Show memory usage
    resp - Manage response from database
    trap - Manage spam trap

Type <! to see this message again.
```
";

pub static RTCMD_USAGE_EMBED: &str = "\
```
embed <u64:ChannelId>
    [--authortext/-at \"<string>\"/me]
        ↳ string <= 256
    [--authorurl/-au <string:url>]
    [--authoravatar/-aa <string:url>/me]
    [--color/-c <string:RRGGBB>]
    [--description/-d \"<string>\"]
        ↳ string <= 4096
    [--field/-f \"<string:name>\" \"<string:value>\" <bool:inline(true|false)>]
        ↳ MAXCOUNT <= 25 | name <= 256 | value <= 1024
    [--footertext/-ft \"<string>\"/me]
        ↳ string <= 2048
    [--footericon/-fi <string:url/me>]
    [--image/-i <string:url>]
    [--thumbnail/-tn <string:url>/me]
    [--timestamp/-ts <string:ISO8601/UNIX>/now/utc]
    [--title/-t \"<string>\"]
        ↳ string <= 256
    [--url/-u <string:url>]

Special values:
    me   → use Your own info (avatar, name)
    now  → current timestamp
    utc  → current timestamp in UTC
```
";

pub static RTCMD_USAGE_RESP: &str = "\
resp <subcommand> [arguments]

resp is a command that manages the response when <\" or 'Rustoki' is called.

Subcommands:
    update
        ↳ Reload all resp data from the database and synchronize memory
        ↳ Usage: resp update

    new_key <string:key>
        ↳ Create a new key with empty responses
        ↳ Fails if the key already exists
        ↳ Usage: resp new_key HELLO

    add_key <string:key> <string:key2>
    add_key <string:key> [\"<string:key2>\", ...]
        ↳ Add one or multiple keys under an existing key
        ↳ Duplicate keys are ignored and produce an error
        ↳ Usage:
            resp add_key HELLO GREET
            resp add_key HELLO [\"GREET\", \"WELCOME\"]

    add <string:key> \"<string:response>\"
    add <string:key> [\"<string:response>\", ...]
        ↳ Add single or multiple responses to a key
        ↳ Usage:
            resp add HELLO hi
            resp add HELLO [\"hi\", \"hey\", \"welcome\"]

    get_table <string:key>
        ↳ Show all responses for the specified key
        ↳ Usage: resp get_table HELLO

    remove <string:key> \"<string:response>\" [-m/--multi]
        ↳ Remove a response from a key
        ↳ If -m/--multi is provided, removes all matching responses
        ↳ Usage:
            resp remove HELLO \"hi\"
            resp remove HELLO \"hi\" -m

    remove_table <string:key> DOUBLECHECK [-f/--force]
        ↳ Remove the key and all associated responses
        ↳ DOUBLECHECK is required for confirmation
        ↳ -f/--force  Bypass additional checks (remove even if the key exists under another ID)
        ↳ Usage: resp remove_table \"hello\" DOUBLECHECK -f
Notes:
    - JSON arrays are supported for bulk operations (e.g., add or add_key)
    - Invalid JSON formats are ignored
    - Commands are case-sensitive
    - It contains an AUTOMATIC STRING PROCESSOR. You must use capital letters WITHOUT spaces or special characters
";
