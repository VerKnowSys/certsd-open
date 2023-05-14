# CertsD

> CertsD-open - open-source, automated, asynchronous LE certificate issuer


# Author:

Daniel ([@dmilith](https://twitter.com/dmilith)) Dettlaff



## Features:

- Generates separate certificates for the root domain and its wildcard version.

- Uses [RON](https://github.com/ron-rs/ron) formatted configuration.

- Supports multiple CloudFlare accounts and multiple domains/ zones at once.

- Automatic management of DNS TXT records via the CloudFlare API.

- Notifies Slack using a Webhook after a successful renewal.

- Asynchronous by default.



## Requirements read from the configuration file:

- CloudFlare API Token (with "Edit zone DNS" permission).

- CloudFlare Zone ID

- A domain



## Step by step how it works

- CertsD reads the input configuration from [one of the existing paths](https://github.com/VerKnowSys/certsd-open/blob/master/src/config.rs#L29-L32).

- The ACME registration process starts in the current working directory.

- Attempt to reuse all non-existent key files (`account.key` + `example.com/domain.key` + `wild_example.com/domain.key`) or generates them automatically.

- Validate the expiration date of both certs (`example.com/chained.pem` and `wild_example.com/chained.pem`). By default, ACME provides certificates valid for 90 days. Based on that CertsD will only renew certificates that have less than 60 days of validity time left.

- ACME process creates the DNS challenge.

- A DNS TXT record for a given domain (with the value of the challenge) is created using CF API.

- Await confirmation of the order from the ACME response.

- A DNS TXT record for a given domain is deleted using CF API.

- After order confirmation, the (`example.com/chained.pem` + `wild_example.com/chained.pem`) are fetched from ACME.



## A few notes about ACME service:

- CertsD stability relies on the stability of ACME services. Don't panic. Be patient.

- From time to time the ACME API responds with a random "invalid" status just because. Don't panic. Be patient.

- If you won't remove one of (`account`.key` + `example.com/domain.key` + `wild_example.com/domain.key`) too often, the ACME is likely to renew your certs faster without any issues (ACME cert caching mechanism).

- If you want to use ACME Staging for testing, set the `acme_staging: true` in your configuration.


## Software requirements:

- Rust >= 1.68.2
- OpenSSL >= 1.1.1t



## Additional build requirements:

- Clang >= 6.x
- Make >= 3.x
- Cmake >= 3.16
- Perl >= 5.x
- Patchelf > 0.17
- POSIX-compliant base-system (tested on systems: FreeBSD/ HardenedBSD/ Darwin and Linux)



# Production Configuration:

> NOTE: I hold the configuration under `/Services/Certsd/service.conf`, all keys and generated certificates under `/Services/Certsd`.

```ron
(
    acme_staging: false,
    accounts: [
        (
            cloudflare_api_token: "cloudflare-api-token",
            cloudflare_zone_id: "cloudflare-zone-id",
            domain: "myexample.com",
            contacts: ["domains@example.com"],
        ),

        // …
    ],

    notifications: [
        Slack(webhook: "https://hooks.slack.com/services/111111111/33333333333/44444444444444444"),
        Telegram(
            chat_id: "@Public_Channel",
            token: "1111111111111111111111111111111"
        ),
        // …
    ]
)
```


# Production cron entry example:

```cron
# run certsd every 10 days, 30 minutes before midnight:
30 23 */10 * * "/Software/Certsd/exports/certsd >> /var/log/renew-example.com.log"
```


# Example Nginx proxy configuration (to serve generated `chained.pem` to remote hosts):

```conf
server {
   listen       80;
   server_name  my.example.com;
   autoindex off;

   location ~ .*/chained.pem {
       root   /etc/certsd/certs;
   }

   location / {
       deny  all;
   }
}
```
