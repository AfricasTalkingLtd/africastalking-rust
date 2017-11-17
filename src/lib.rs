// #![deny(missing_docs,
//         missing_debug_implementations, missing_copy_implementations,
//         trivial_casts, trivial_numeric_casts,
//         unsafe_code,
//         unstable_features,
//         unused_import_braces, unused_qualifications)]
//
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate hyper;
extern crate reqwest;
extern crate serde;
#[macro_use]
extern crate serde_json as json;

use std::io::Read;
use std::collections::HashMap;

use serde::ser::Serialize;
use hyper::header::{Accept, Headers};
header! { (Apikey, "apikey") => [String] }

#[allow(unused_variables)]
trait HttpAccessMethods {
    fn send_request(&self, url: &str, data: Option<HashMap<&str, &str>>) {}
}

error_chain! {
    foreign_links {
        Network(reqwest::Error);
        Io(::std::io::Error);
        Json(json::Error);
    }
    errors {
        GatewayError(e: String){
            description("Gateway error"),
            display("{}", e),
        } }

}

#[allow(unused_variables)]
trait UserData {
    fn get_user_data(&self) {}
}

#[derive(Debug)]
pub struct AfricasTalkingGateway {
    username: String,
    api_key: String,
    env: String,
    user_data_url: String,
    sms_url: String,
    voice_url: String,
    sms_subscription_url: String,
    send_airtime_url: String,
    mobi_payment_checkout_url: String,
    mobi_payment_b2c_url: String,
    mobi_payment_b2b_url: String,
}

impl AfricasTalkingGateway {
    pub fn new(username: &str, api_key: &str, env: &str) -> Self {
        let api_host = if env == "sandbox" {
            "https://api.sandbox.africastalking.com"
        } else {
            "https://api.africastalking.com"
        };
        let voice_host = if env == "sandbox" {
            "https://voice.sandbox.africastalking.com"
        } else {
            "https://voice.africastalking.com"
        };
        let payments_host = if env == "sandbox" {
            "https://payments.sandbox.africastalking.com"
        } else {
            "https://payments.africastalking.com"
        };

        Self {
            username: username.into(),
            api_key: api_key.into(),
            env: env.into(),
            user_data_url: format!("{}/version1/user", api_host),
            sms_url: format!("{}/version1/messaging", api_host),
            voice_url: format!("{}", voice_host),
            sms_subscription_url: format!("{}/version1/subscription", api_host),
            send_airtime_url: format!("{}/version1/airtime/send", api_host),
            mobi_payment_checkout_url: format!("{}/mobile/checkout/request", payments_host),
            mobi_payment_b2c_url: format!("{}/mobile/b2c/request", payments_host),
            mobi_payment_b2b_url: format!("{}/mobile/b2b/request", payments_host),
        }
    }

    pub fn get_user_data(&self) -> Result<json::Value> {
        let url = format!("{}?username={}", self.user_data_url, self.username);
        let val: json::Value = self.send_request(&url, None)?;

        Ok(val)
    }

    #[allow(unused_variables)]
    pub fn send_message(
        &self,
        to: &str,
        message: &str,
        from: &str,
        bulk_sms_mode: bool,
        enqueue: i32,
        keyword: &str,
        link_id: &str,
        retry_duration_in_hours: i32,
    ) -> Result<json::Value> {
        let params = json!({
            "username": self.username,
            "to": to,
            "message": message,
            "bulkSMSMode": bulk_sms_mode as i32
        });

        let mut resp = self.send_form_data(&self.sms_url, params)?;
        let mut buf = String::new();
        resp.read_to_string(&mut buf)?;

        let val: json::Value = json::from_str(&buf)?;

        Ok(val)
    }

    fn send_request(&self, url: &str, data: Option<HashMap<&str, &str>>) -> Result<json::Value> {
        let mut headers = Headers::new();
        headers.set(Accept::json());
        headers.set(Apikey(self.api_key.clone()));
        let client = reqwest::Client::new();
        let mut resp = match data {
            Some(map) => client.post(url).json(&map).send()?,
            None => client.get(url).headers(headers).send()?,
        };

        Ok(resp.json()?)
    }


    fn send_form_data<T: Serialize>(&self, url: &str, data: T) -> Result<reqwest::Response> {
        let mut headers = Headers::new();
        headers.set(Accept::json());
        headers.set(Apikey(self.api_key.clone()));
        let client = reqwest::Client::new();
        let resp = client.post(url).form(&data).headers(headers).send()?;

        Ok(resp)
    }

    fn send_json_request<T: Serialize>(&self, url: &str, data: T) -> Result<reqwest::Response> {
        let mut headers = Headers::new();
        headers.set(Accept::json());
        headers.set(Apikey(self.api_key.clone()));
        let client = reqwest::Client::new();
        let resp = client
            .post(url)
            .json(&data)
            .headers(headers)
            .send()?;

        Ok(resp)
    }

    /// Sends airtime. [docs reference](http://docs.africastalking.com/airtime/sending)
    ///
    /// `recipients` is a json array of the format
    ///
    /// ```json,ignore
    /// [
    ///   {
    ///     "phoneNumber":"+254711XXXYYY",
    ///     "amount":"KES X"
    ///   },
    ///   {
    ///     "phoneNumber":"+254733YYYZZZ",
    ///     "amount":"KES Y"
    ///   }
    /// ]
    /// ```
    pub fn send_airtime(&self, recipients: json::Value) -> Result<json::Value> {
        let params = json!({
            "username": self.username,
            "recipients": recipients
        });
        let mut resp = self.send_form_data(&self.send_airtime_url, params)?;
        if resp.status().as_u16() == 201 {
            let jsn: json::Value = resp.json()?;
            let responses: json::Value = jsn.get("responses").unwrap().clone();
            if jsn["responses"].as_array().unwrap().len() > 0 {
                return Ok(responses);
            } else {
                // raise error
                Err(ErrorKind::GatewayError(format!("{}", jsn["errorMessage"])).into())
            }
        } else {
            // raise error
            Err(ErrorKind::GatewayError(format!("{}", resp.text()?)).into())
        }
    }

    ///  Initiates a checkout request on a subscriber's phone number.
    ///  [read more ..](http://docs.africastalking.com/mobile/checkout)
    pub fn init_mobile_payment_checkout(
        &self,
        product_name: &str,
        phone_number: &str,
        currency_code: &str,
        provider_channel: &str,
        amount: f32,
        metadata: HashMap<&str, &str>,
    ) -> Result<json::Value> {
        let params = json!({
            "username": self.username,
            "productName": product_name,
            "phoneNumber": phone_number,
            "currencyCode": currency_code,
            "providerChannel": provider_channel,
            "amount": amount,
            "metadata": metadata
        });
        let mut resp = self.send_json_request(&self.mobi_payment_checkout_url, Some(params))?;
        if resp.status().as_u16() == 201 {
            let jsn: json::Value = resp.json()?;
            let entries: json::Value = jsn.get("entries").unwrap().clone();
            if jsn["entries"].as_array().unwrap().len() > 0 {
                return Ok(entries);
            } else {
                // raise error
                Err(ErrorKind::GatewayError(format!("{}", jsn["errorMessage"])).into())
            }
        } else {
            // raise error
            Err(ErrorKind::GatewayError(format!("{}", resp.text()?)).into())
        }
    }

    /// Requests a Business-to-Business payment to a business via their provider channel.
    /// [read more..](http://docs.africastalking.com/mobile/b2b)
    pub fn mobile_payment_b2b_request(
        &self,
        product_name: &str,
        provider_data: HashMap<&str, &str>,
        currency_code: &str,
        amount: f32,
        metadata: HashMap<&str, &str>,
    ) -> Result<json::Value> {
        for field in vec![
            "provider",
            "destination_channel",
            "destination_account",
            "transfer_type",
        ] {
            assert!(
                provider_data.contains_key(field),
                format!("Missing field {} in provider data", field)
            );
        }

        let params = json!({
            "username": self.username,
            "productName": product_name,
            "provider": provider_data.get("provider").unwrap(),
            "destinationChannel": provider_data.get("destination_channel").unwrap(),
            "destinationAccount": provider_data.get("destination_account").unwrap(),
            "transferType": provider_data.get("transfer_type").unwrap(),
            "currencyCode": currency_code,
            "amount": amount,
            "metadata": metadata
        });

        let mut resp = self.send_json_request(&self.mobi_payment_b2b_url, Some(params))?;
        if resp.status().as_u16() == 201 {
            let jsn: json::Value = resp.json()?;
            Ok(jsn)
        } else {
            // raise error
            Err(ErrorKind::GatewayError(format!("{:?}", resp)).into())
        }
    }

    /// Requests a Business-to-Consumer payment to  mobile subscribers phone numbers.
    /// [read more..](http://docs.africastalking.com/mobile/b2c)
    pub fn mobile_payment_b2c_request(
        &self,
        product_name: &str,
        recipients: json::Value,
    ) -> Result<json::Value> {
        assert!(
            recipients.as_array().unwrap().len() <= 10,
            "Recipients should not be greater than 10"
        );
        let params = json!({
            "username": self.username,
            "productName": product_name,
            "recipients": recipients
        });

        let mut resp = self.send_json_request(&self.mobi_payment_b2c_url, Some(params))?;
        if resp.status().as_u16() == 201 {
            let jsn: json::Value = resp.json()?;
            let entries: json::Value = jsn.get("entries").unwrap().clone();
            if jsn["entries"].as_array().unwrap().len() > 0 {
                return Ok(entries);
            } else {
                Err(ErrorKind::GatewayError(format!("{}", jsn["errorMessage"])).into())
            }
        } else {
            Err(ErrorKind::GatewayError(format!("{:?}", resp.text()?)).into())
        }
    }
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
