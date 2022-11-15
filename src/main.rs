use std::env;

use rand::Rng;
use serenity::async_trait;
use serenity::model::gateway::Ready;
use serenity::model::application::interaction::Interaction;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::prelude::command::{Command, CommandOptionType};
use serenity::model::prelude::interaction::InteractionResponseType;
use serenity::model::prelude::interaction::application_command::CommandDataOptionValue;
use serenity::prelude::*;
use serenity::framework::standard::StandardFramework;
use songbird::{SerenityInit, create_player, ytdl};

#[derive(Default)]
struct Discobot;

impl Discobot {
    async fn play(ctx: &Context, cmd: &ApplicationCommandInteraction) {
        let song_url = match cmd.data.options.get(0).unwrap().resolved.as_ref().unwrap() {
            CommandDataOptionValue::String(str) => str,
            _ => "",
        };

        let guild_id = cmd.guild_id.expect("Failed to get guild id from cache!");
        let guild = &ctx.cache.guild(guild_id).unwrap();
        let manager = songbird::serenity::get(&ctx).await.expect("Failed to get manager.");
        let call_lock = manager.get_or_insert(guild_id);
        let mut call = call_lock.lock().await;
        let channel_id = guild.voice_states.get(&cmd.user.id)
            .and_then(|voice_state| voice_state.channel_id).unwrap();
        call.join(channel_id).await.unwrap();

        let source = match ytdl(song_url).await {
            Ok(s) => s,
            Err(why) => {
                Discobot::create_reply(format!("Failed to get YT song: {}", why).as_str(), ctx, cmd).await;
                return;
            }
        };

        let (track, handle) = create_player(source);

        let index = call.queue().len();
        call.enqueue(track);

        Discobot::create_reply(
            format!("Added {} to the Queue! Index: {}", handle.metadata().title.clone().unwrap_or("Unkown".to_string()), index).as_str(),
            ctx, cmd).await;
    }

    async fn clear(ctx: &Context, cmd: &ApplicationCommandInteraction) {
        let guild_id = cmd.guild_id.expect("Failed to get guild id from cache!");
        let manager = songbird::serenity::get(&ctx).await.expect("Failed to get manager.");
        let call_lock = manager.get_or_insert(guild_id);
        let mut call = call_lock.lock().await;
        call.stop();
        call.queue().modify_queue(|q| {
            q.clear()
        });
        Discobot::create_reply("Playlist cleared!", ctx, cmd).await;
    }

    async fn skip(ctx: &Context, cmd: &ApplicationCommandInteraction) {
        let guild_id = cmd.guild_id.expect("Failed to get guild id from cache!");
        let manager = songbird::serenity::get(&ctx).await.expect("Failed to get manager.");
        let call_lock = manager.get_or_insert(guild_id);
        let call = call_lock.lock().await;
        if let Err(why) = call.queue().skip() {
            Discobot::create_reply(format!("Failed to skip: {}", why).as_str(), ctx, cmd).await;
            return;
        }
        Discobot::create_reply("Skipped!", ctx, cmd).await;
    }

    async fn resume(ctx: &Context, cmd: &ApplicationCommandInteraction) {
        let guild_id = cmd.guild_id.expect("Failed to get guild id from cache!");
        let manager = songbird::serenity::get(&ctx).await.expect("Failed to get manager.");
        let call_lock = manager.get_or_insert(guild_id);
        let call = call_lock.lock().await;
        if let Some(song) = call.queue().current() {
            if let Err(why) = song.play() {
                Discobot::create_reply(format!("Failed to play: {}", why).as_str(), ctx, cmd).await;
                return;
            }
        }
        Discobot::create_reply("Resumed!", ctx, cmd).await;
    }

    async fn pause(ctx: &Context, cmd: &ApplicationCommandInteraction) {
        let guild_id = cmd.guild_id.expect("Failed to get guild id from cache!");
        let manager = songbird::serenity::get(&ctx).await.expect("Failed to get manager.");
        let call_lock = manager.get_or_insert(guild_id);
        let call = call_lock.lock().await;
        if let Some(song) = call.queue().current() {
            if let Err(why) = song.pause() {
                Discobot::create_reply(format!("Failed to pause: {}", why).as_str(), ctx, cmd).await;
                return;
            }
        }
        Discobot::create_reply("Paused!", ctx, cmd).await;
    }

    async fn remove(ctx: &Context, cmd: &ApplicationCommandInteraction) {
        match cmd.data.options.get(0).unwrap().resolved.as_ref().unwrap() {
            CommandDataOptionValue::Integer(int) => {
                let guild_id = cmd.guild_id.expect("Failed to get guild id from cache!");
                let manager = songbird::serenity::get(&ctx).await.expect("Failed to get manager.");
                let call_lock = manager.get_or_insert(guild_id);
                let call = call_lock.lock().await;

                call.queue().modify_queue(|q| {
                    if let Some(s) = q.remove(*int as usize) {
                        let _ = Discobot::create_reply(format!("Removed {} to the Queue! Index: {}", s.metadata().title.clone().unwrap_or("Unkown".to_string()), int).as_str(), ctx, cmd);
                    } else {
                        let _ =Discobot::create_reply("Couldn't remove song.", ctx, cmd);
                    }
                })
            }
            _ => {},
        };
    }

    async fn playlist(ctx: &Context, cmd: &ApplicationCommandInteraction) {
        let guild_id = cmd.guild_id.expect("Failed to get guild id from cache!");
        let manager = songbird::serenity::get(&ctx).await.expect("Failed to get manager.");
        let call_lock = manager.get_or_insert(guild_id);
        let call = call_lock.lock().await;
        let playlist = call.queue();

        let mut rply = String::from("Current Playlist:\n");
        for i in 0..playlist.len() {
            rply.push_str(format!("{}: {}\n", i, playlist.current_queue()[i].metadata().title.clone().unwrap_or("Unknown Song".to_string())).as_str());

        }
        Discobot::create_reply(rply.as_str(), ctx, cmd).await;
    }

    async fn roll(ctx: &Context, cmd: &ApplicationCommandInteraction) {
        let dices = match cmd.data.options.get(0).unwrap().resolved.as_ref().unwrap() {
            CommandDataOptionValue::Integer(num) => num,
            _ => &1,
        };
        let sides = match cmd.data.options.get(1).unwrap().resolved.as_ref().unwrap() {
            CommandDataOptionValue::Integer(num) => num,
            _ => &6,
        };

        let mut total_str: String = String::from("You rolled ");

        let mut total: Vec<i64> = Vec::new();
        let mut sum = 0;

        for _ in 1..dices + 1 {
            let num = rand::thread_rng().gen_range(1..sides + 1);
            sum += num;
            total.push(num);
            total_str.push_str(&format!("{}, ", num).to_string());
        }

        total_str.push_str(&format!("total: {}.", sum).to_string());

        Discobot::create_reply(total_str.as_str(), ctx, cmd).await
    }

    async fn create_reply(msg: &str, ctx: &Context, cmd: &ApplicationCommandInteraction) {
        if let Err(why) = cmd.create_interaction_response(&ctx.http, |response| {
            response.kind(InteractionResponseType::ChannelMessageWithSource).interaction_response_data(|message| {
                message.content(msg)
            })
        }).await {
            println!("Cannot respont do slash: {}", why);
        }
    }

}

#[async_trait]
impl EventHandler for Discobot {
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command)=  interaction {
            match command.data.name.as_str() {
                "play" => Discobot::play(&ctx, &command).await,
                "resume" => Discobot::resume(&ctx, &command).await,
                "pause" => Discobot::pause(&ctx, &command).await,
                "roll" => Discobot::roll(&ctx, &command).await,
                "remove" => Discobot::remove(&ctx, &command).await,
                "skip" => Discobot::skip(&ctx, &command).await,
                "clear" => Discobot::clear(&ctx, &command).await,
                "playlist" => Discobot::playlist(&ctx, &command).await,
                _ => {},
            };
        };
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);

        let cmds = Command::set_global_application_commands(&ctx.http, |cmds| {
            cmds
                .create_application_command(|cmd|
                                            cmd
                                            .name("play")
                                            .description("Play a song!")
                                            .create_option(|opt|
                                                           opt
                                                           .name("song")
                                                           .description("The song name or url")
                                                           .kind(CommandOptionType::String)
                                                           .required(true)))
                .create_application_command(|cmd|
                                            cmd
                                            .name("remove")
                                            .description("Remove a song from the playlist.")
                                            .create_option(|opt|
                                                           opt
                                                           .name("song_index")
                                                           .description("Index of the song")
                                                           .kind(CommandOptionType::Integer)
                                                           .required(true)))
                .create_application_command(|cmd|
                                            cmd
                                            .name("playlist")
                                            .description("Shows the current playlist."))
                .create_application_command(|cmd|
                                            cmd
                                            .name("skip")
                                            .description("Skip current song."))
                .create_application_command(|cmd|
                                            cmd
                                            .name("clear")
                                            .description("Clear the queue"))
                .create_application_command(|cmd|
                                            cmd
                                            .name("pause")
                                            .description("Pauses the player"))
                .create_application_command(|cmd|
                                            cmd
                                            .name("resume")
                                            .description("Resumes the player"))
                .create_application_command(|cmd|
                                            cmd
                                            .name("roll")
                                            .description("Rolls XdY.")
                                            .create_option(|opt|
                                                           opt
                                                           .name("dices")
                                                           .description("Number of dice.")
                                                           .kind(CommandOptionType::Integer)
                                                           .required(true))
                                            .create_option(|opt|
                                                           opt
                                                           .name("sides")
                                                           .description("Number of sides.")
                                                           .kind(CommandOptionType::Integer)
                                                           .required(true)))
        }).await;

        println!("Created the following commands: {:#?}", cmds);
    }
}

#[tokio::main]
async fn main() {
    let framework = StandardFramework::new();

    let discobot: Discobot = Discobot::default();

    // Configure the client with your Discord bot token in the environment.
    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::non_privileged()
        | GatewayIntents::MESSAGE_CONTENT;

    // Create a new instance of the Client, logging in as a bot.
    let mut client = Client::builder(&token, intents)
        .event_handler(discobot)
        .framework(framework)
        .register_songbird()
        .await
        .expect("Err creating client");

    // Finally, start a single shard, and start listening to events.
    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
