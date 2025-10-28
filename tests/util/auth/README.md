# Generating RSA Keys for Testing Token Validation in Callback

1. Generate the private key with:

```sh
openssl genpkey -algorithm RSA -out private_test_rsa_key.pem
```

2. Generate the public key with:

```sh
openssl rsa -in private_test_rsa_key.pem -pubout -out public_test_rsa_key.pem
```
