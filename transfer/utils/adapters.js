const task = require("tasuku");
const { getPriorityFee } = require("./priorityFee");
const { getNumberDecimals } = require("./decimals");
const { TransactionMessage } = require("@solana/web3.js");
const { ComputeBudgetProgram } = require("@solana/web3.js");
const { VersionedTransaction } = require("@solana/web3.js");
const { createTransferInstruction } = require("@solana/spl-token");
const { getOrCreateAssociatedTokenAccount } = require("@solana/spl-token");

class Adapter {
  constructor(connection, fromKeypair, splAddress, beneficiary) {
    this.connection = connection;
    this.fromKeypair = fromKeypair;
    this.splAddress = splAddress;
    this.beneficiary = beneficiary;
  }

  async transfer() {
    await task("", async ({ setTitle, setError }) => {
      try {
        const priorityFee = await getPriorityFee();

        // Instruction to set the compute unit price for priority fee
        const PRIORITY_FEE_INSTRUCTIONS =
          ComputeBudgetProgram.setComputeUnitPrice({
            microLamports: BigInt(Math.floor(priorityFee)),
          });

        // Get decimals for SPL token address
        const decimals = await getNumberDecimals(
          this.splAddress,
          this.connection
        );

        // Creates or fetches the associated token accounts for the sender and receiver
        let sourceAccount = await getOrCreateAssociatedTokenAccount(
          this.connection,
          this.fromKeypair,
          this.splAddress,
          this.fromKeypair.publicKey
        );

        let beneficiaryAccount = await getOrCreateAssociatedTokenAccount(
          this.connection,
          this.fromKeypair,
          this.splAddress,
          this.beneficiary
        );

        // Adjusts the transfer amount according to the token's decimals to ensure accurate transfers.
        const transferAmountInDecimals = Number(sourceAccount.amount);
        setTitle(
          `${this.fromKeypair.publicKey} balance is ${
            transferAmountInDecimals / Math.pow(10, decimals)
          }`
        );

        // Balance is 0, return
        if (transferAmountInDecimals === 0) {
          setTitle(`${this.fromKeypair.publicKey} Transfer end.`);
          setError("Balance is 0");
          return;
        }

        setTitle(
          `${this.fromKeypair.publicKey} start transfer ${
            transferAmountInDecimals / Math.pow(10, decimals)
          } to ${this.beneficiary.toString().slice(0, 4)}...${this.beneficiary
            .toString()
            .slice(-4)}`
        );

        // Prepares the transfer instructions with all necessary information.
        const transferInstruction = createTransferInstruction(
          // Those addresses are the Associated Token Accounts belonging to the sender and receiver
          sourceAccount.address,
          beneficiaryAccount.address,
          this.fromKeypair.publicKey,
          transferAmountInDecimals
        );

        let latestBlockhash = await this.connection.getLatestBlockhash(
          "confirmed"
        );

        // Compiles and signs the transaction message with the sender's Keypair.
        const messageV0 = new TransactionMessage({
          payerKey: this.fromKeypair.publicKey,
          recentBlockhash: latestBlockhash.blockhash,
          instructions: [PRIORITY_FEE_INSTRUCTIONS, transferInstruction],
        }).compileToV0Message();

        const versionedTransaction = new VersionedTransaction(messageV0);
        versionedTransaction.sign([this.fromKeypair]);
        setTitle("Transaction Signed. Preparing to send...");

        // Attempts to send the transaction to the network, handling success or failure.
        try {
          const txid = await this.connection.sendTransaction(
            versionedTransaction,
            {
              maxRetries: 10,
            }
          );
          setTitle(`Transaction Submitted: ${txid}`);

          const confirmation = await this.connection.confirmTransaction(
            {
              signature: txid,
              blockhash: latestBlockhash.blockhash,
              lastValidBlockHeight: latestBlockhash.lastValidBlockHeight,
            },
            "confirmed"
          );
          if (confirmation.value.err) {
            setError("🚨Transaction not confirmed.");
          }
          setTitle(
            `Transaction Successfully Confirmed! 🎉 View on SolScan: https://solscan.io/tx/${txid}`
          );
        } catch (error) {
          setError("Transaction failed", error);
        }
      } catch (err) {
        setError(err);
      }
      setTimeout(() => console.log("Timeout executed"), 1500);
    });
  }

  async amount() {
    const sourceAccount = await getOrCreateAssociatedTokenAccount(
      this.connection,
      this.fromKeypair,
      this.splAddress,
      this.fromKeypair.publicKey
    );

    const transferAmountInDecimals = Number(sourceAccount.amount);

    return transferAmountInDecimals;
  }
}

module.exports = {
  Adapter,
};
