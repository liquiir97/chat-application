use std::{collections::HashSet, sync::{Arc, Mutex}};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

struct AppState
{
    user_set : Mutex<HashSet<String>>,
    tx: broadcast::Sender<String>,
}

#[tokio::main]
async fn main() {
    
    tracing_subscriber::registry().with(
        tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "example_chat=trace".into()),
    ).with(tracing_subscriber::fmt::layer())
    .init();

    let user_set = Mutex::new(HashSet::new());
    let (tx, _rx) = broadcast::channel(100);

    let app_state = Arc::new(AppState { user_set, tx });

    let app = Router::new()
        .route("/", get(index))
        .route("/websocket", get(websocket_handler))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
    .await
    .unwrap();

    tracing::debug!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();

}

async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| websocket(socket, state))
}

async fn websocket(stream: WebSocket, state : Arc<AppState>)
{
    let(mut sender,  mut receiver) = stream.split();

    let mut username = String::new();

    while let Some(Ok(message)) = receiver.next().await {
        if let Message::Text(name) = message {
            // If username that is sent by client is not taken, fill username string.
            let result = check_username(&state, &mut username, &name);

            
            // If not empty we want to quit the loop else we want to quit function.
            if result.eq("OK")
            {
                break;
            }
            else
            {
                let _ = sender
                .send(Message::Text(String::from("Username already taken.")))
                .await;

                return;
            }
            // if !username.is_empty() {
            //     break;
            // } else {
            //     // Only send our client that username is taken.
            //     let _ = sender
            //         .send(Message::Text(String::from("Username already taken.")))
            //         .await;

            //     return;
            // }
        }
    }
    // while let Some(Ok(message)) = reciver.next().await
    // {
    //     if let Message::Text(name) = message
    //     {
    //         check_username(&state, &mut username, &name);

    //         if !username.is_empty()
    //         {
    //             break;
    //         }
    //         else
    //         {
    //             let _  = sender.send(Message::Text(String::from("Username already taken."))).await;
    //             return;
    //         }
    //     }
    // }

    let mut rx = state.tx.subscribe();

    let msg = format!("{username} joined.");
    tracing::debug!("{msg}");
    let _ = state.tx.send(msg);

    let mut send_task = tokio::spawn(async move
    {
        while let Ok(msg) = rx.recv().await
        {
            if sender.send(Message::Text(msg)).await.is_err()
            {
                break;
            }
        }
    });

    let tx = state.tx.clone();
    let name = username.clone();

    let mut recv_task = tokio::spawn(async move
    {
        while let Some(Ok(Message::Text(text))) = receiver.next().await
        {
            let _  = tx.send(format!("{name}: {text}"));
        }
    });

    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    };

    let msg = format!("{username} left.");
    tracing::debug!("{msg}");
    let _ = state.tx.send(msg);

    state.user_set.lock().unwrap().remove(&username);

}

fn check_username(state : &AppState, string : &mut String, name : &str) -> String
{
    let mut user_set = state.user_set.lock().unwrap();

    let usrnames = format!("{:?}", user_set);
    tracing::debug!("USERNAMES");
    tracing::debug!("{usrnames}");
    if(!user_set.contains(name))
    {
        user_set.insert(name.to_owned());
        string.push_str(name);
        "OK".to_string()
    }
    else
    {
        string.push_str(name);
        "NOK".to_string()
    }
}

async fn index() -> Html<&'static str> {
    Html(std::include_str!("../chat.html"))
}
