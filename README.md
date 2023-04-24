# CertsD

> CertsD-open - open-source, automated, asynchronous LE certificate issuer


# Author:

Daniel ([@dmilith](https://twitter.com/dmilith)) Dettlaff



# Features:

- Generates separate certificates for the root domain and its wildcard version.

- Uses [RON](https://github.com/ron-rs/ron) formatted configuration.

- Supports multiple CloudFlare accounts and multiple domains/ zones at once.

- Automatic management of DNS TXT records via the CloudFlare API.

- Notifies Slack using a Webhook after a successful renewal.

- Asynchronous by default (except the Slack notification part).



# Requirements read from the configuration file:

- CloudFlare API Token (with "Edit zone DNS" permission).

- CloudFlare Zone ID

- A domain



# Step by step how it works

- CertsD reads the input configuration from [one of the existing paths](https://github.com/VerKnowSys/certsd-open/blob/master/src/config.rs#L29-L32).

- The ACME registration process starts in the current working directory.

- Attempt to reuse all non-existent key files (`account.key` + `example.com/domain.key` + `wild_example.com/domain.key`) or generates them automatically.

- Validate the expiration date of both certs (`example.com/chained.pem` and `wild_example.com/chained.pem`). By default, ACME provides certificates valid for 90 days. Based on that CertsD will only renew certificates that have less than 60 days of validity time.

- ACME process creates the DNS challenge.

- A DNS TXT record for a given domain (with the value of the challenge) is created using CF API.

- Await the ACME response. After the order is confirmed, the (`example.com/chained.pem` + `wild_example.com/chained.pem`) are created.




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
    ],

    slack_webhook: "https://hooks.slack.com/services/AAAAAAAAAAA/AAAAAAAAAAA/AAAAAAAAAAAAAAAAAAAAAA",
)
```


# Example Nginx proxy configuration (to serve generated `chained.pem` to remote hosts):

```conf
server {
   listen       80;
   server_name  my.example.com;
   autoindex off;

   location ~ .*/chained.pem {
       root   /var/www/certsd;
   }

   location / {
       deny  all;
   }
}
```
