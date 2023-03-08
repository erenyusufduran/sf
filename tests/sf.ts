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
  const [solBankPDA, solBankBump] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("sol_bank")],
    program.programId
  );

  describe("Contract Tests", () => {
    beforeEach(async () => {
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
    });

    it("Initialize SolBank, Stage and Admin accounts.", async () => {
      await program.methods
        .stage(solBankBump)
        .accounts({
          management: management.publicKey,
          solBank: solBankPDA,
          systemProgram: SystemProgram.programId,
        })
        .signers([management])
        .rpc();

      const _solBank = await program.account.solBank.fetch(solBankPDA);
      const _management = await program.account.management.fetch(management.publicKey);

      assert(_solBank, solBankPDA.toString());
      assert(provider.wallet.publicKey.toString(), _management.admin.toString());
      assert(_management.executed, true.toString());
    });

    it("When pause false, process begins.", async () => {
      await program.methods
        .pause(false)
        .accounts({ management: management.publicKey, admin: provider.wallet.publicKey })
        .rpc();

      const _management = await program.account.management.fetch(management.publicKey);
      assert(!_management.pause);
    });

    it("Create community", async () => {
      const arr = [];
      for (let i = 0; i < 5; i++) {
        const newUser = anchor.web3.Keypair.generate().publicKey;
        arr.push(newUser);
      }
      const community = anchor.web3.Keypair.generate();
      const _comm = {
        members: arr,
        name: "SF",
        description: "This is a social funding platform for social communities.",
        permission: false,
      };
      await program.methods
        .createCommunity(_comm.name, _comm.description, _comm.members, _comm.permission)
        .accounts({
          community: new anchor.web3.PublicKey(community.publicKey),
          systemProgram: SystemProgram.programId,
        })
        .signers([community])
        .rpc();

      const communityAccount = await program.account.community.fetch(community.publicKey);
      assert(communityAccount.name, _comm.name);
      assert(communityAccount.description, _comm.description);
    });
  });
});
