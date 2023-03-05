use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::{invoke, invoke_signed};
use errors::ErrorCode;
use state::*;

pub mod errors;
pub mod state;
declare_id!("FCLsYGgoyrfT6MjftqVTFs3kCwc9HC8hRfYyYGPi85QE");

#[program]
pub mod sf {
    use super::*;

    pub fn stage(ctx: Context<Stage>, sol_bank_bump: u8) -> Result<()> {
        let stage = &mut ctx.accounts.management;
        let admin = &mut ctx.accounts.admin;
        let sol_bank = &mut ctx.accounts.sol_bank;

        require!(stage.admin == admin.key(), ErrorCode::AuthenticationError);

        require!(stage.executed != true, ErrorCode::AlreadyExecuted);

        stage.executed = true;
        stage.admin = admin.key();
        stage.projects_count = 0;
        stage.is_fund_distributed = false;
        stage.multiple = 0 as f64;

        sol_bank.amount = 0;
        sol_bank.bump = sol_bank_bump;

        sol_bank.projects = Vec::new();
        sol_bank.project_counts = Vec::new();
        sol_bank.project_amounts = Vec::new();

        Ok(())
    }

    pub fn pause(ctx: Context<Pause>, pause: bool) -> Result<()> {
        let stage = &mut ctx.accounts.management;
        let admin = &mut ctx.accounts.admin;

        stage.pause = pause;

        require!(stage.admin == admin.key(), ErrorCode::AuthenticationError);

        if !pause {
            let clock = Clock::get().unwrap();

            const ONE_DAY: i64 = 60 * 60 * 24;

            stage.project_stage = clock.unix_timestamp;
            stage.voting_stage = stage.project_stage + ONE_DAY * 3;
            stage.execute_stage = stage.voting_stage + ONE_DAY * 5;
            stage.donate_stage = stage.execute_stage + ONE_DAY;
            stage.distribute_stage = stage.donate_stage + ONE_DAY * 10;

            stage.is_fund_distributed = false;
        }
        Ok(())
    }

    pub fn create_community(
        ctx: Context<CreateCommunity>,
        name: String,
        description: String,
        members: Vec<Pubkey>,
        permission: bool,
    ) -> Result<()> {
        let community = &mut ctx.accounts.community;
        let user = &ctx.accounts.user;
        let clock = Clock::get().unwrap();

        require!(members.len() >= 4, ErrorCode::InsufficientNumber);

        for member in members {
            community.members.push(member);
        }

        // userı signer olarak alıyoruz onun için vektöre userın keyini atıyoruz
        // members parametresinden ise direkt pubkey alıyoruz
        community.members.push(user.key());
        community.timestamp = clock.unix_timestamp;

        community.name = name;
        community.description = description;
        community.permission = permission;

        Ok(())
    }

    pub fn join_community(ctx: Context<JoinCommunity>) -> Result<()> {
        let community = &mut ctx.accounts.community;
        let user = &ctx.accounts.user;
        let member_counter = &mut ctx.accounts.member_counter;

        if community.permission == false {
            community.members.push(user.key());
        } else {
            community.members_pool.push(user.key());
            member_counter.counter = 0;
        }

        Ok(())
    }

    pub fn add_member_to_community(ctx: Context<AddMembertoCommunity>) -> Result<()> {
        let community = &mut ctx.accounts.community;
        let member_counter = &mut ctx.accounts.member_counter;
        let user = &mut ctx.accounts.user;

        let mut is_this_member = false;
        for member in community.members.iter() {
            if &user.key() == member {
                is_this_member = true;
                break;
            }
        }

        require!(is_this_member, ErrorCode::AuthenticationError);
        member_counter.counter += 1;

        if member_counter.counter > (community.members.len() / 5) as i64 {
            community.members.push(user.key());
        }

        Ok(())
    }

    pub fn create_project(
        ctx: Context<CreateProject>,
        subject: String,
        description: String,
    ) -> Result<()> {
        let project = &mut ctx.accounts.project;
        let management = &mut ctx.accounts.management;
        let creator = &ctx.accounts.creator;
        let community = &mut ctx.accounts.community;
        let counter = &mut ctx.accounts.counter;

        let clock = Clock::get().unwrap();

        require!(!management.pause, ErrorCode::ContractPause);
        require!(
            management.project_stage < clock.unix_timestamp
                && management.voting_stage > clock.unix_timestamp,
            ErrorCode::NotInProjectStage
        );

        let mut is_this_member = false;
        for member in community.members.iter() {
            if &creator.key() == member {
                is_this_member = true;
                break;
            }
        }

        require!(is_this_member, ErrorCode::AuthenticationError);

        counter.no_count = 0;
        counter.yes_count = 0;

        project.subject = subject;
        project.description = description;
        project.creator = *creator.key;
        project.community = community.key();
        project.executable = false;

        management.projects_count += 1;

        Ok(())
    }

    pub fn vote(ctx: Context<Vote>, vote: String, voting_bump: u8) -> Result<()> {
        let voting = &mut ctx.accounts.voting;
        let project = &mut ctx.accounts.project;
        let management = &mut ctx.accounts.management;
        let counter = &mut ctx.accounts.counter;
        let community = &mut ctx.accounts.community;
        let user = &mut ctx.accounts.user;

        let clock = Clock::get().unwrap();
        require!(!management.pause, ErrorCode::ContractPause);

        let mut is_this_member = false;
        for member in community.members.iter() {
            if &user.key() == member {
                is_this_member = true;
                break;
            }
        }
        require!(is_this_member, ErrorCode::AuthenticationError);

        require!(
            management.voting_stage < clock.unix_timestamp
                && management.execute_stage > clock.unix_timestamp,
            ErrorCode::NotInVotingStage
        );

        let voting_char = VotingResult::validate(vote.chars().nth(0).unwrap());
        require!(voting_char != VotingResult::Invalid, ErrorCode::InvalidChar);

        if voting_char == VotingResult::Yes {
            counter.yes_count += 1;
        } else {
            counter.no_count += 1;
        }

        voting.user = *user.key;
        voting.project = project.key();
        voting.timestamp = clock.unix_timestamp;
        voting.result = voting_char;
        voting.bump = voting_bump;

        Ok(())
    }

    pub fn execute_project(ctx: Context<ExecuteProject>) -> Result<()> {
        let project = &mut ctx.accounts.project;
        let management = &mut ctx.accounts.management;
        let creator = &ctx.accounts.creator;
        let community = &mut ctx.accounts.community;
        let counter = &mut ctx.accounts.counter;
        let sol_bank = &mut ctx.accounts.sol_bank;

        let clock = Clock::get().unwrap();
        require!(!management.pause, ErrorCode::ContractPause);

        require!(
            management.execute_stage < clock.unix_timestamp
                && management.donate_stage > clock.unix_timestamp,
            ErrorCode::NotInExecuteStage
        );

        require!(
            creator.key() == project.creator,
            ErrorCode::AuthenticationError
        );

        if counter.yes_count > counter.no_count
            && counter.yes_count + counter.no_count > (community.members.len() / 2) as i64
        {
            project.executable = true;
        }
        require!(project.executable, ErrorCode::NotPublish);

        sol_bank.projects.push(project.key());

        Ok(())
    }

    pub fn donate_project(
        ctx: Context<DonateProject>,
        _donate: u64,
        donate_bump: u8,
    ) -> Result<()> {
        let donate = &mut ctx.accounts.donate;
        let management = &mut ctx.accounts.management;
        let user = &mut ctx.accounts.user;
        let sol_bank = &mut ctx.accounts.sol_bank;
        let project = &mut ctx.accounts.project;

        let clock = Clock::get().unwrap();
        require!(!management.pause, ErrorCode::ContractPause);

        require!(
            management.donate_stage < clock.unix_timestamp
                && management.distribute_stage > clock.unix_timestamp,
            ErrorCode::NotInDonateStage
        );

        let transfer_sol = anchor_lang::solana_program::system_instruction::transfer(
            &user.key(),
            &sol_bank.key(),
            _donate,
        );

        invoke(
            &transfer_sol,
            &[
                user.to_account_info(),
                sol_bank.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
        )?;

        let index = sol_bank.projects.iter().position(|&r| r == project.key());

        sol_bank.project_amounts[index.unwrap()] += _donate;
        sol_bank.project_counts[index.unwrap()] += 1;

        sol_bank.amount += _donate;
        donate.donate_count += 1;
        donate.amount += _donate;
        sol_bank.sol_counter += 1;

        donate.donate_bump = donate_bump;

        Ok(())
    }

    pub fn distribute_funds(ctx: Context<DistributeFunds>) -> Result<()> {
        let sol_bank = &mut ctx.accounts.sol_bank;
        let management = &mut ctx.accounts.management;
        let user = &ctx.accounts.user;
        let donate = &mut ctx.accounts.donate;
        let project = &mut ctx.accounts.project;

        let clock = Clock::get().unwrap();

        require!(
            user.key() == management.admin,
            ErrorCode::AuthenticationError
        );

        require!(
            management.distribute_stage < clock.unix_timestamp,
            ErrorCode::NotInDistributeStage
        );

        require!(
            management.is_fund_distributed == false,
            ErrorCode::NotOpenedYet
        );

        if management.multiple == 0 as f64 {
            let mut sum = 0 as f64;
            for i in 0..management.projects_count {
                sum += (sol_bank.project_amounts[i as usize] * sol_bank.project_counts[i as usize])
                    as f64;
            }
            management.multiple = sol_bank.amount as f64 / sum;
        }

        let funded_amount = (donate.donate_count * donate.amount) * management.multiple as u64;

        let transfer_sol = anchor_lang::solana_program::system_instruction::transfer(
            &sol_bank.key(),
            &project.key(),
            funded_amount,
        );

        invoke_signed(
            &transfer_sol,
            &[
                sol_bank.to_account_info(),
                project.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[&[&[sol_bank.bump]]],
        )?;
        Ok(())
    }

    pub fn ask_for_withdraw(
        ctx: Context<AskForWithdraw>,
        amount: u64,
        withdraw_bump: u8,
    ) -> Result<()> {
        let withdraw = &mut ctx.accounts.withdraw;
        let user = &mut ctx.accounts.user;
        let counter = &mut ctx.accounts.counter;
        let community = &mut ctx.accounts.community;
        let donate = &mut ctx.accounts.donate;

        let mut is_this_member = false;
        for member in community.members.iter() {
            if &user.key() == member {
                is_this_member = true;
                break;
            }
        }
        require!(is_this_member, ErrorCode::AuthenticationError);

        require!(amount < donate.amount, ErrorCode::InsufficientError);

        counter.no_count = 0;
        counter.yes_count = 0;

        withdraw.user = *user.key;
        withdraw.amount = amount;
        withdraw.bump = withdraw_bump;

        withdraw.executable = false;
        withdraw.executed = false;

        Ok(())
    }

    pub fn voting_withdraw(
        ctx: Context<VotingWithdraw>,
        vote: String,
        withdraw_bump: u8,
    ) -> Result<()> {
        let withdraw = &mut ctx.accounts.withdraw;
        let user = &mut ctx.accounts.user;
        let community = &mut ctx.accounts.community;
        let counter = &mut ctx.accounts.counter;

        let mut is_this_member = false;
        for member in community.members.iter() {
            if &user.key() == member {
                is_this_member = true;
                break;
            }
        }

        require!(is_this_member, ErrorCode::AuthenticationError);

        let voting_char = VotingResult::validate(vote.chars().nth(0).unwrap());
        require!(voting_char != VotingResult::Invalid, ErrorCode::InvalidChar);

        if voting_char == VotingResult::Yes {
            counter.yes_count += 1;
        } else {
            counter.no_count += 1;
        }

        if counter.yes_count > (counter.no_count + counter.yes_count) * 60 / 100
            && counter.yes_count + counter.no_count > (community.members.len() / 2) as i64
        {
            withdraw.executable = true;
        }

        withdraw.bump = withdraw_bump;

        Ok(())
    }

    pub fn withdraw(ctx: Context<VotingWithdraw>) -> Result<()> {
        let withdraw = &mut ctx.accounts.withdraw;
        let user = &mut ctx.accounts.user;
        let project = &mut ctx.accounts.project;

        require!(user.key() == withdraw.user, ErrorCode::AuthenticationError);
        require!(withdraw.executed == false, ErrorCode::AlreadyExecuted);

        if withdraw.executable == true {
            withdraw.executed = true;

            let transfer_sol = anchor_lang::solana_program::system_instruction::transfer(
                &project.key(),
                &user.key(),
                withdraw.amount,
            );

            invoke(
                &transfer_sol,
                &[
                    project.to_account_info(),
                    user.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                ],
            )?;
        }

        Ok(())
    }
}
