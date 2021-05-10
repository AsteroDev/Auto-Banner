use std::{error::Error, env};
use futures::stream::StreamExt;
use std::convert::TryFrom;
use std::time::Instant;
use twilight_cache_inmemory::{InMemoryCache, ResourceType};
use twilight_gateway::{cluster::{Cluster, ShardScheme}, Event};
use twilight_http::{Client as HttpClient, request::AuditLogReason};
use twilight_model::gateway::{payload::update_status::UpdateStatusInfo, presence::Status, Intents};
use twilight_util::snowflake::Snowflake;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let scheme = ShardScheme::Auto;
    let token = env::var("DISCORD_TOKEN")?;
    let intents = Intents::GUILD_MEMBERS;

    let cluster = Cluster::builder(&token, intents)
        .shard_scheme(scheme)
        .presence(UpdateStatusInfo {
            activities: None,
            since: None,
            status: Status::Offline,
            afk: false
        })
        .build()
        .await?;

    let cluster_spawn = cluster.clone();

    tokio::spawn(async move {
        cluster_spawn.up().await;
    });

    let http = HttpClient::new(&token);

    let cache = InMemoryCache::builder()
        .resource_types(ResourceType::MEMBER)
        .build();

    let mut events = cluster.events();

    while let Some((shard_id, event)) = events.next().await {
        cache.update(&event);
        tokio::spawn(handle_event(shard_id, event, http.clone()));
    }

    Ok(())
}

async fn handle_event(
    shard_id: u64,
    event: Event,
    http: HttpClient,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match event {
        Event::MemberAdd(member) => {
            let username = member.user.name.to_lowercase();
            let reason = "User is most likely a spam account.";
            let now = Instant::now().elapsed().as_millis();

            if username.contains("/token") || username.contains("john f") || username.contains("motion") || now + 60000 > u128::try_from(member.user.id.timestamp())? {
                let ban = http
                  .create_ban(member.guild_id, member.user.id)
                  .delete_message_days(7)?
                  .reason(reason)?
                  .await;

                match ban {
                    Ok(()) => println!("Banned {}", format!("{}#{}", member.user.name, member.user.discriminator)),
                    Err(err) => println!("{}", err),
                }

                return Ok(());
            }
        }
        Event::ShardConnected(_) => {
            let user = http.current_user().await?;

            println!("Shard {} connected with user {}", shard_id, format!("{}#{}", user.name, user.discriminator));
        }
        _ => {}
    }

    Ok(())
}