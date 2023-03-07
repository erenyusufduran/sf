import * as anchor from "@project-serum/anchor";
import { Program } from "@project-serum/anchor";
import { Sf } from "../target/types/sf";
import { assert } from "chai";

describe("sf", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Sf as Program<Sf>;
  const { SystemProgram } = anchor.web3;

  const management = anchor.web3.Keypair.generate();
  const admin = provider.wallet;
  it("Initialize SolBank, Stage and Admin accounts.", async () => {
    const [solBankPDA, solBankBump] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("sol_bank")],
      program.programId
    );

    const balance = await provider.connection.getBalance(admin.publicKey);
    if (balance / anchor.web3.LAMPORTS_PER_SOL < 1) {
      const airdropSign = await provider.connection.requestAirdrop(admin.publicKey, anchor.web3.LAMPORTS_PER_SOL);
      const latestBlockhash = await provider.connection.getLatestBlockhash();
      await provider.connection.confirmTransaction({
        blockhash: latestBlockhash.blockhash,
        lastValidBlockHeight: latestBlockhash.lastValidBlockHeight,
        signature: airdropSign,
      });
    }
    console.log("New balance is: ", await provider.connection.getBalance(admin.publicKey));

    await program.methods
      .stage(solBankBump)
      .accounts({
        management: management.publicKey,
        solBank: solBankPDA,
        admin: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
      })
      .signers([management])
      // .signers([admin, management])
      .rpc();

    const _solBank = await program.account.solBank.fetch(solBankPDA);
    const _management = await program.account.management.fetch(management.publicKey);

    assert(_solBank, solBankPDA.toString());
    assert(provider.wallet.publicKey.toString(), _management.admin.toString());
    assert(_management.executed, true.toString());
  });
});
