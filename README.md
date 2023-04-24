
## Example Nginx configuration (to serve generated `chained.pem`):

```conf
server {
   listen       80;
   server_name  my.example.hub.com;
   autoindex off;

   location ~ .*/chained.pem {
       root   /Volumes/Projects/certsd;
   }

   location / {
       deny  all;
   }
}
```
