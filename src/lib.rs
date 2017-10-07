//! # Mammut: API Wrapper around the Mastodon API.
//!
//! Most of the api is documented on [Mastodon's
//! github](https://github.com/tootsuite/mastodon/blob/master/docs/Using-the-API/API.md#tag)
//!
//! ```no_run
//! # extern crate mammut;
//! # fn main() {
//! #    try().unwrap();
//! # }
//! # fn try() -> mammut::Result<()> {
//! use mammut::Registration;
//! use mammut::apps::{AppBuilder, Scope};
//!
//! let app = AppBuilder {
//!     client_name: "mammut_test",
//!     redirect_uris: "urn:ietf:wg:oauth:2.0:oob",
//!     scopes: Scope::Read,
//!     website: None,
//! };
//!
//! let mut registration = Registration::new("https://mastodon.social")?;
//! registration.register(app)?;
//! let url = registration.authorise()?;
//! // Here you now need to open the url in the browser
//! // And handle a the redirect url coming back with the code.
//! let code = String::from("RETURNED_FROM_BROWSER");
//! let mastodon = registration.create_access_token(code)?;
//!
//! println!("{:?}", mastodon.get_home_timeline()?);
//! # Ok(())
//! # }
//! ```

#![cfg_attr(test, deny(warnings))]

#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json as json;
extern crate chrono;
extern crate reqwest;
extern crate serde;
extern crate url;
use std::str;

extern crate websocket;
use std::thread;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use websocket::OwnedMessage;
use websocket::client::ClientBuilder;


/// Registering your App
pub mod apps;
/// Constructing a status
pub mod status_builder;
/// Entities returned from the API
pub mod entities;
/// Registering your app.
pub mod registration;

use std::ops;
use std::io::Error as IoError;

use json::Error as SerdeError;
use reqwest::Error as HttpError;
use reqwest::Client;
use reqwest::header::{Authorization, Bearer, Headers};

use entities::prelude::*;
pub use status_builder::StatusBuilder;

pub use registration::Registration;
pub type Result<T> = std::result::Result<T, Error>;

macro_rules! methods {
    ($($method:ident,)+) => {
        $(
            fn $method<T: for<'de> serde::Deserialize<'de>>(&self, url: String)
            -> Result<T>
            {
                use std::io::Read;

                let mut response = self.client.$method(&url)
                    .headers(self.headers.clone())
                    .send()?;

                let mut vec = Vec::new();
                response.read_to_end(&mut vec)?;

                if let Ok(t) = json::from_slice(&vec) {
                    Ok(t)
                } else {
                    Err(Error::Api(json::from_slice(&vec)?))
                }
            }
         )+
    };
}

macro_rules! route {

    ((post ($($param:ident: $typ:ty,)*)) $name:ident: $url:expr => $ret:ty, $($rest:tt)*) => {
        /// Equivalent to `/api/v1/
        #[doc = $url]
        /// `
        ///
        #[doc = "# Errors"]
        /// If `access_token` is not set.
        pub fn $name(&self, $($param: $typ,)*) -> Result<$ret> {
            use std::io::Read;

            let form_data = json!({
                $(
                    stringify!($param): $param,
                    )*
            });

            let mut response = self.client.post(&self.route(concat!("/api/v1/", $url)))
                .headers(self.headers.clone())
                .form(&form_data)
                .send()?;

            let mut vec = Vec::new();

            response.read_to_end(&mut vec)?;

            if let Ok(t) = json::from_slice(&vec) {
                Ok(t)
            } else {
                Err(Error::Api(json::from_slice(&vec)?))
            }
        }
        route!{$($rest)*}
    };

    (($method:ident) $name:ident: $url:expr => $ret:ty, $($rest:tt)*) => {
        /// Equivalent to `/api/v1/
        #[doc = $url]
        /// `
        ///
        #[doc = "# Errors"]
        /// If `access_token` is not set.
        pub fn $name(&self) -> Result<$ret> {
            self.$method(self.route(concat!("/api/v1/", $url)))
        }

        route!{$($rest)*}
    };

    () => {}
}

macro_rules! route_id {

    ($(($method:ident) $name:ident: $url:expr => $ret:ty,)*) => {
        $(
            /// Equivalent to `/api/v1/
            #[doc = $url]
            /// `
            ///
            #[doc = "# Errors"]
            /// If `access_token` is not set.
            pub fn $name(&self, id: u64) -> Result<$ret> {
                self.$method(self.route(&format!(concat!("/api/v1/", $url), id)))
            }
         )*
    }

}

#[derive(Clone, Debug)]
pub struct Mastodon {
    client: Client,
    headers: Headers,
    /// Raw data about your mastodon instance.
    pub data: Data,
}

/// Raw data about mastodon app. Save `Data` using `serde` to prevent needing
/// to authenticate on every run.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Data {
    pub base: String,
    pub client_id: String,
    pub client_secret: String,
    pub redirect: String,
    pub token: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Error {
    Api(ApiError),
    #[serde(skip_deserializing)]
    Serde(SerdeError),
    #[serde(skip_deserializing)]
    Http(HttpError),
    #[serde(skip_deserializing)]
    Io(IoError),
    #[serde(skip_deserializing)]
    ClientIdRequired,
    #[serde(skip_deserializing)]
    ClientSecretRequired,
    #[serde(skip_deserializing)]
    AccessTokenRequired,
}

/// Error returned from the Mastodon API.
#[derive(Clone, Debug, Deserialize)]
pub struct ApiError {
    /// The type of error.
    pub error: String,
    /// The description of the error.
    pub error_description: Option<String>,
}

impl Mastodon {
    fn from_registration(
        base: String,
        client_id: String,
        client_secret: String,
        redirect: String,
        token: String,
        client: Client,
    ) -> Self {
        let data = Data {
            base: base,
            client_id: client_id,
            client_secret: client_secret,
            redirect: redirect,
            token: token,
        };

        let mut headers = Headers::new();
        headers.set(Authorization(Bearer { token: data.token.clone() }));

        Mastodon {
            client: client,
            headers: headers,
            data: data,
        }
    }

    /// Creates a mastodon instance from the data struct.
    pub fn from_data(data: Data) -> Result<Self> {
        let mut headers = Headers::new();
        headers.set(Authorization(Bearer { token: data.token.clone() }));

        Ok(Mastodon {
            client: Client::new()?,
            headers: headers,
            data: data,
        })
    }

    route! {
        (get) verify: "accounts/verify_credentials" => Account,
        (get) blocks: "blocks" => Vec<Account>,
        (get) follow_requests: "follow_requests" => Vec<Account>,
        (get) mutes: "mutes" => Vec<Account>,
        (get) notifications: "notifications" => Vec<Notification>,
        (get) reports: "reports" => Vec<Report>,
        (get) get_home_timeline: "timelines/home" => Vec<Status>,
        (post (id: u64,)) allow_follow_request: "accounts/follow_requests/authorize" => Empty,
        (post (id: u64,)) reject_follow_request: "accounts/follow_requests/reject" => Empty,
        (post (uri: String,)) follows: "follows" => Account,
        (post) clear_notifications: "notifications/clear" => Empty,
        (post (file: Vec<u8>,)) media: "media" => Attachment,
        (post (account_id: u64, status_ids: Vec<u64>, comment: String,)) report:
            "reports" => Report,
        (post (q: String, resolve: bool,)) search: "search" => SearchResult,
    }

    route_id! {
        (get) get_account: "accounts/{}" => Account,
        (get) followers: "accounts/{}/followers" => Vec<Account>,
        (get) following: "accounts/{}/following" => Vec<Account>,
        (get) follow: "accounts/{}/follow" => Account,
        (get) unfollow: "accounts/{}/unfollow" => Account,
        (get) block: "accounts/{}/block" => Account,
        (get) unblock: "accounts/{}/unblock" => Account,
        (get) mute: "accounts/{}/mute" => Account,
        (get) unmute: "accounts/{}/unmute" => Account,
        (get) get_notification: "notifications/{}" => Notification,
        (get) get_status: "statuses/{}" => Status,
        (get) get_context: "statuses/{}/context" => Context,
        (get) get_card: "statuses/{}/card" => Card,
        (get) reblogged_by: "statuses/{}/reblogged_by" => Vec<Account>,
        (get) favourited_by: "statuses/{}/favourited_by" => Vec<Account>,
        (post) reblog: "statuses/{}/reblog" => Status,
        (post) unreblog: "statuses/{}/unreblog" => Status,
        (post) favourite: "statuses/{}/favourite" => Status,
        (post) unfavourite: "statuses/{}/unfavourite" => Status,
        (delete) delete_status: "statuses/{}" => Empty,
    }

    pub fn new_status(&self, status: StatusBuilder) -> Result<Status> {
        use std::io::Read;

        let mut response = self.client
            .post(&self.route("/api/v1/statuses"))
            .headers(self.headers.clone())
            .json(&status)
            .send()?;

        let mut vec = Vec::new();
        response.read_to_end(&mut vec)?;

        if let Ok(t) = json::from_slice(&vec) {
            Ok(t)
        } else {
            Err(Error::Api(json::from_slice(&vec)?))
        }
    }

    pub fn get_public_timeline(&self, local: bool) -> Result<Vec<Status>> {
        let mut url = self.route("/api/v1/timelines/public");

        if local {
            url += "?local=1";
        }

        self.get(url)
    }

    pub fn get_tagged_timeline(&self, hashtag: String, local: bool) -> Result<Vec<Status>> {
        let mut url = self.route("/api/v1/timelines/tag/");
        url += &hashtag;

        if local {
            url += "?local=1";
        }

        self.get(url)
    }

    pub fn statuses(
        &self,
        id: u64,
        only_media: bool,
        exclude_replies: bool,
    ) -> Result<Vec<Status>> {
        let mut url = format!("{}/api/v1/accounts/{}/statuses", self.base, id);

        if only_media {
            url += "?only_media=1";
        }

        if exclude_replies {
            url += if only_media { "&" } else { "?" };

            url += "exclude_replies=1";
        }

        self.get(url)
    }


    pub fn relationships(&self, ids: &[u64]) -> Result<Vec<Relationship>> {
        let mut url = self.route("/api/v1/accounts/relationships?");

        if ids.len() == 1 {
            url += "id=";
            url += &ids[0].to_string();
        } else {
            for id in ids {
                url += "id[]=";
                url += &id.to_string();
                url += "&";
            }
            url.pop();
        }

        self.get(url)
    }

    // TODO: Add a limit fn
    pub fn search_accounts(&self, query: &str) -> Result<Vec<Account>> {
        self.get(format!("{}/api/v1/accounts/search?q={}", self.base, query))
    }

    pub fn instance(&self) -> Result<Instance> {
        self.get(self.route("/api/v1/instance"))
    }

    methods![get, post, delete,];

    fn route(&self, url: &str) -> String {
        let mut s = self.base.clone();
        s += url;
        s
    }

    pub fn get_user_streaming(&self) -> (Receiver<Status>, Receiver<Notification>) {

        let domain = url::Url::parse(&self.data.base).unwrap();
        let url = format!(
            "wss://{}/api/v1/streaming/?access_token={}&stream=user",
            domain.host_str().unwrap(),
            self.data.token
        );
        println!("start connection {}", url);

        let mut headers = Headers::new();
        headers.set(Authorization(format!("Bearer {}", self.data.token)));

        let mut client = ClientBuilder::new(&url)
            .unwrap()
            .add_protocol("rust-websocket")
            .custom_headers(&headers)
            .connect_secure(None)
            .unwrap();


        let (status_tx, status_rx) = channel();
        let (notification_tx, notification_rx) = channel();

        let status_tx_1 = status_tx.clone();
        let notification_tx_1 = notification_tx.clone();

        thread::spawn(move || for message in client.incoming_messages() {
            let message_opt: Option<OwnedMessage> = match message {
                Ok(m) => Some(m),
                Err(e) => {
                    println!("error: {:?}", e);
                    None
                }
            };
            match message_opt {
                Some(OwnedMessage::Close(_)) => {
                    return;
                }
                Some(OwnedMessage::Ping(_)) => {}
                Some(OwnedMessage::Text(text)) => {

                    let text_opt: Option<String> = Some(text);

                    match text_opt {
                        Some(text) => {
                            let ws_event_opt: json::Result<Event> = json::from_str(&text);
                            match ws_event_opt {
                                Ok(ref event) if event.event == "update" => {
                                    let status_opt: json::Result<Status> = json::from_str(&event.payload);
                                    match status_opt {
                                        Ok(status) => {
                                            let _ = status_tx_1.send(status);
                                        }
                                        Err(e) => {
                                            println!("error: status parse error => {}", e);
                                        }
                                    };
                                }
                                Ok(ref event) if event.event == "notification" => {
                                    let notification_opt: json::Result<Notification> = json::from_str(&event.payload);
                                    match notification_opt {
                                        Ok(notification) => {
                                            let _ = notification_tx_1.send(notification);
                                        }
                                        Err(e) => {
                                            println!("error: notification parse error => {}", e);
                                        }
                                    };
                                }
                                Ok(_) => {
                                    println!("error: unknown payload {:?}", ws_event_opt);
                                }
                                Err(e) => {
                                    println!("error: status parse error => {}", e);
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => println!("Receive Loop: {:?}", message_opt),
            };
            println!("break out incoming messages loop");
        });

        (status_rx, notification_rx)
    }
}

impl ops::Deref for Mastodon {
    type Target = Data;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

macro_rules! from {
    ($($typ:ident, $variant:ident,)*) => {
        $(
            impl From<$typ> for Error {
                fn from(from: $typ) -> Self {
                    use Error::*;
                    $variant(from)
                }
            }
        )*
    }
}

from! {
    SerdeError, Serde,
    HttpError, Http,
    IoError, Io,
}
