use reqwest::Client;

use super::{Error, Mastodon, Result};
use apps::{AppBuilder, Scope};

/// Handles registering your mastodon app to your instance. It is recommended
/// you cache your data struct to avoid registering on every run.
pub struct Registration {
    base: String,
    client: Client,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub redirect: Option<String>,
    scopes: Scope,
}

#[derive(Deserialize)]
struct OAuth {
    client_id: String,
    client_secret: String,
    redirect_uri: String,
}

#[derive(Deserialize)]
struct AccessToken {
    access_token: String,
}

impl Registration {
    pub fn new<I: Into<String>>(base: I) -> Result<Self> {
        Ok(Registration {
            base: base.into(),
            client: Client::new()?,
            client_id: None,
            client_secret: None,
            redirect: None,
            scopes: Scope::Read,
        })
    }

    /// Register the application with the server from the `base` url.
    ///
    /// ```no_run
    /// # extern crate mammut;
    /// # fn main() {
    /// #    try().unwrap();
    /// # }
    /// # fn try() -> mammut::Result<()> {
    /// use mammut::Registration;
    /// use mammut::apps::{AppBuilder, Scope};
    ///
    /// let app = AppBuilder {
    ///     client_name: "mammut_test",
    ///     redirect_uris: "urn:ietf:wg:oauth:2.0:oob",
    ///     scopes: Scope::Read,
    ///     website: None,
    /// };
    ///
    /// let mut registration = Registration::new("https://mastodon.social")?;
    /// registration.register(app)?;
    /// let url = registration.authorise()?;
    /// // Here you now need to open the url in the browser
    /// // And handle a the redirect url coming back with the code.
    /// let code = String::from("RETURNED_FROM_BROWSER");
    /// let mastodon = registration.create_access_token(code)?;
    ///
    /// println!("{:?}", mastodon.get_home_timeline()?);
    /// # Ok(())
    /// # }
    /// ```
    pub fn register(&mut self, app_builder: AppBuilder) -> Result<()> {
        let url = format!("{}/api/v1/apps", self.base);
        self.scopes = app_builder.scopes;

        let app: OAuth = self.client.post(&url).form(&app_builder).send()?.json()?;

        self.client_id = Some(app.client_id);
        self.client_secret = Some(app.client_secret);
        self.redirect = Some(app.redirect_uri);

        Ok(())
    }

    /// Returns the full url needed for authorisation. This needs to be opened
    /// in a browser.
    pub fn authorise(&mut self) -> Result<String> {
        self.is_registered()?;

        let url = format!(
            "{}/oauth/authorize?client_id={}&redirect_uri={}&scope={}&response_type=code",
            self.base,
            self.client_id.clone().unwrap(),
            self.redirect.clone().unwrap(),
            self.scopes,
        );

        Ok(url)
    }

    fn is_registered(&self) -> Result<()> {
        if self.client_id.is_none() {
            Err(Error::ClientIdRequired)
        } else if self.client_secret.is_none() {
            Err(Error::ClientSecretRequired)
        } else {
            Ok(())
        }
    }

    pub fn create_access_token(self, code: String) -> Result<Mastodon> {
        self.is_registered()?;
        let url = format!(
            "{}/oauth/token?client_id={}&client_secret={}&code={}&grant_type=authorization_code&redirect_uri={}",
            self.base,
            self.client_id.clone().unwrap(),
            self.client_secret.clone().unwrap(),
            code,
            self.redirect.clone().unwrap()
        );

        let token: AccessToken = self.client.post(&url).send()?.json()?;

        Ok(Mastodon::from_registration(self.base,
                                       self.client_id.unwrap(),
                                       self.client_secret.unwrap(),
                                       self.redirect.unwrap(),
                                       token.access_token,
                                       self.client))
    }

    pub fn set_access_token(self, access_token: String) -> Result<Mastodon> {
        Ok(Mastodon::from_registration(self.base,
                                       self.client_id.unwrap(),
                                       self.client_secret.unwrap(),
                                       self.redirect.unwrap(),
                                       access_token,
                                       self.client))
    }
}


