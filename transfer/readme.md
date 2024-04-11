## Overview🚀

This repository provides a simple **spl token** script to transfer Solana tokens using private keys stored in a file named `PrivateKeys.txt`. Each line in the file corresponds to a single private key in the bs58 format. The script reads these private keys, connects to the Solana network, and transfers the specified amount of the designated SPL token to a predefined beneficiary address.

## Prerequisites

Before running the script, make sure you have the following prerequisites installed:

- Node.js (version 16 or higher)
- npm (Node Package Manager)

## Setup

1. Clone this repository to your local machine.

2. Navigate to the repository directory.

3. Install dependencies by running `npm install`.

4. Open the `PrivateKeys.txt` file and write your bs58-formatted private keys, with each private key on a separate line.

5. Open the 

   ```
   config.js
   ```

    file and specify your configuration:

   - `rpcUrl`: Solana RPC URL
   - `wsEndpoint`: Solana WebSocket endpoint
   - `splToken`: SPL token address
   - `beneficiary`: Beneficiary address

6. Save the changes to `config.js`.

## Usage

To start the token transfer process, execute the following command in your terminal:

```
node main.js
```

This command will initiate the transfer process using the private keys and configurations specified in the `PrivateKeys.txt` and `config.js` files respectively.

## Important Note

Ensure that you have sufficient Solana tokens in the specified SPL token address for successful transfers. Additionally, review and confirm the correctness of the beneficiary address before executing the script.