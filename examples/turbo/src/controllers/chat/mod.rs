//! Chat controller

use std::collections::HashMap;

use crate::models::{ChatMessage, User};
use rwf::prelude::*;

mod form;
use form::MessageForm;

#[derive(Clone, rwf::macros::TemplateValue)]
struct UserMessage {
    user: User,
    message: ChatMessage,
    mine: bool,
}

#[derive(rwf::macros::Context)]
struct Context {
    title: String,
    messages: Vec<UserMessage>,
    user: User,
}

#[derive(rwf::macros::PageController)]
#[auth(auth)]
pub struct ChatController {
    auth: AuthHandler,
}

impl ChatController {
    pub fn new() -> Self {
        Self {
            auth: SessionAuth::redirect("/signup").handler(),
        }
    }

    fn chat_message(user: &User, message: &ChatMessage, mine: bool) -> Result<TurboStream, Error> {
        let chat_message = Template::load("templates/chat_message.html")?;
        let rendered = chat_message.render([(
            "message",
            UserMessage {
                user: user.clone(),
                message: message.clone(),
                mine,
            }
            .to_template_value()?,
        )])?;

        Ok(TurboStream::new(rendered)
            .action("append")
            .target("messages"))
    }

    // fn typing(user: &User, action: &str) -> Result<TurboStream, Error> {
    //     let typing = Template::load("templates/typing.html")?;
    //     let rendered = typing.render([("user", user.clone().to_template_value()?)])?;

    //     Ok(TurboStream::new(rendered)
    //         .action(action)
    //         .target(format!("typing-{}", user.id.unwrap())))
    // }
}

#[rwf::async_trait]
impl PageController for ChatController {
    async fn get(&self, request: &Request) -> Result<Response, Error> {
        let mut conn = Pool::connection().await?;

        let user = if let Some(id) = request.user_id() {
            User::find(id).fetch(&mut conn).await?
        } else {
            return Ok(Response::new().redirect("/signup"));
        };

        let users = User::all().fetch_all(&mut conn).await?;
        let messages = User::related::<ChatMessage>(&users)
            .order("id")
            .fetch_all(&mut conn)
            .await?;

        let users = users
            .into_iter()
            .map(|user| (user.id.unwrap(), user))
            .collect::<HashMap<_, _>>();

        let messages = messages
            .into_iter()
            .map(|message| UserMessage {
                user: users[&message.user_id].clone(),
                mine: users[&message.user_id].id() == user.id(),
                message,
            })
            .collect::<Vec<_>>();

        let rendered = Template::load("templates/chat.html")?.render(Context {
            title: "rwf + Turbo = chat".into(),
            messages,
            user,
        })?;

        Ok(Response::new().html(rendered))
    }

    async fn post(&self, request: &Request) -> Result<Response, Error> {
        let form = request.form::<MessageForm>()?;

        if form.body.is_empty() {
            return Ok(Response::bad_request());
        }

        let mut conn = Pool::connection().await?;

        let user = if let Some(user_id) = request.user_id() {
            User::find(user_id).fetch(&mut conn).await?
        } else {
            return Ok(Response::new().redirect("/signup"));
        };

        let message =
            ChatMessage::create(&[("body", form.body.to_value()), ("user_id", user.id())])
                .fetch(&mut conn)
                .await?;

        // Broadcast the message to everyone else.
        if let Some(session_id) = request.session_id() {
            let broadcast = Comms::broadcast(&session_id);
            let message = Self::chat_message(&user, &message, false)?.render();
            broadcast.send(Message::Text(message))?;

            // let typing = Self::typing(&user, "remove")?;
            // broadcast.send(Message::Text(typing.render()))?;
        }

        // Display the message for the user.
        let chat_message = Self::chat_message(&user, &message, true)?;

        // Reset the form.
        let form = Template::load("templates/chat_form.html")?;
        let form = form.render([("user", user.to_template_value()?)])?;

        Ok(Response::new().turbo_stream(&[
            chat_message,
            TurboStream::new(form).action("replace").target("form"),
        ]))
    }
}
