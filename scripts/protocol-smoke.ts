#!/usr/bin/env bun

import { spawn } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

type JsonObject = Record<string, unknown>;

const SERVER_EXIT_TIMEOUT_MILLIS = 5_000;
const MIN_ANCHOR_TUTORIAL_SMOKE_FIXTURES = 3;

type LspMessage = {
  jsonrpc?: "2.0";
  id?: number | string | null;
  method?: string;
  params?: unknown;
  result?: unknown;
  error?: {
    code: number;
    message: string;
  };
};

type PendingRequest = {
  resolveResponse: (value: unknown) => void;
  reject: (reason: Error) => void;
  timeout: ReturnType<typeof setTimeout>;
};

type Position = {
  line: number;
  character: number;
};

type Range = {
  start: Position;
  end: Position;
};

type Diagnostic = {
  code?: string | number;
  message?: string;
  range: Range;
  relatedInformation?: {
    message?: string;
    location?: {
      uri?: string;
      range?: Range;
    };
  }[];
  data?: JsonObject;
};

type DiagnosticReport = {
  items?: Diagnostic[];
};

type SmokeFixture = {
  name: string;
  uri: string;
  source: string;
};

type TextEdit = {
  range: Range;
  newText: string;
};

type WorkspaceEdit = {
  changes?: Record<string, TextEdit[]>;
};

type PrepareRename = {
  range?: Range;
  placeholder?: string;
};

type CodeAction = {
  title?: string;
  kind?: string;
  diagnostics?: Diagnostic[];
  edit?: WorkspaceEdit;
  isPreferred?: boolean;
  data?: JsonObject;
};

type Hover = {
  contents?: {
    value?: string;
  };
};

type CompletionItem = {
  label?: string;
  detail?: string;
  documentation?: string | {
    value?: string;
  };
  data?: JsonObject;
};

type CompletionResponse =
  | CompletionItem[]
  | {
      items?: CompletionItem[];
    };

type CompletionProvider = {
  triggerCharacters?: string[];
};

type AnalysisReport = {
  uri?: string;
  project?: JsonObject | null;
  evidence?: unknown;
  diagnostics?: unknown[];
  focus?: JsonObject | null;
};

type DocumentSymbol = {
  name?: string;
  children?: DocumentSymbol[];
};

type SymbolInformation = {
  name?: string;
  location?: Location;
};

type DocumentLink = {
  target?: string;
  tooltip?: string;
  data?: JsonObject;
};

type DocumentHighlight = {
  range?: Range;
};

type SignatureHelp = {
  signatures?: {
    label?: string;
  }[];
};

type SelectionRange = {
  range: Range;
  parent?: SelectionRange;
};

type SemanticTokens = {
  data?: unknown[];
};

type FoldingRange = {
  startLine?: number;
  endLine?: number;
  collapsedText?: string;
};

type CodeLens = {
  range: Range;
  command?: {
    title?: string;
    command?: string;
    arguments?: unknown[];
  };
  data?: JsonObject;
};

type Location = {
  uri: string;
  range: Range;
};

type InitializeResult = {
  capabilities?: {
    diagnosticProvider?: unknown;
    completionProvider?: CompletionProvider;
    hoverProvider?: unknown;
    documentSymbolProvider?: unknown;
    documentLinkProvider?: unknown;
    workspaceSymbolProvider?: unknown;
    declarationProvider?: unknown;
    definitionProvider?: unknown;
    typeDefinitionProvider?: unknown;
    implementationProvider?: unknown;
    referencesProvider?: unknown;
    renameProvider?: unknown;
    codeLensProvider?: unknown;
    documentHighlightProvider?: unknown;
    selectionRangeProvider?: unknown;
    foldingRangeProvider?: unknown;
    signatureHelpProvider?: unknown;
    semanticTokensProvider?: unknown;
    inlayHintProvider?: unknown;
    executeCommandProvider?: {
      commands?: string[];
    };
  };
};

type ProposedAssist = {
  id?: string;
  title?: string;
  kind?: string;
  applicability?: string;
  hasEdit?: boolean;
  edit?: WorkspaceEdit;
  evidence?: JsonObject;
};

type ProposedAssistsResponse = {
  uri?: string;
  assists?: ProposedAssist[];
};

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const smokeFixtureRoot = resolve(repoRoot, "target/seagrass-smoke-fixtures");
const artifactSmokeRoot = resolve(repoRoot, "target/seagrass-artifact-smoke");
const artifactSmokeLibPath = resolve(artifactSmokeRoot, "programs/artifact-demo/src/lib.rs");
const artifactSmokeAnchorTomlPath = resolve(artifactSmokeRoot, "Anchor.toml");
const artifactSmokeUri = pathToFileURL(artifactSmokeLibPath).href;
const checkCfgSmokeRoot = resolve(repoRoot, "target/seagrass-check-cfg-smoke");
const anchorDebugSmokeManifestPath = resolve(checkCfgSmokeRoot, "anchor-debug/Cargo.toml");
const anchorDebugSmokeLibPath = resolve(checkCfgSmokeRoot, "anchor-debug/src/lib.rs");
const solanaTargetSmokeManifestPath = resolve(checkCfgSmokeRoot, "solana-target/Cargo.toml");
const solanaTargetSmokeLibPath = resolve(checkCfgSmokeRoot, "solana-target/src/lib.rs");
const anchorDebugSmokeManifestUri = pathToFileURL(anchorDebugSmokeManifestPath).href;
const anchorDebugSmokeUri = pathToFileURL(anchorDebugSmokeLibPath).href;
const solanaTargetSmokeManifestUri = pathToFileURL(solanaTargetSmokeManifestPath).href;
const solanaTargetSmokeUri = pathToFileURL(solanaTargetSmokeLibPath).href;
const checkCfgSmokeWorkspaceUri = pathToFileURL(checkCfgSmokeRoot).href;
const cargoArtifactSmokeRoot = resolve(repoRoot, "target/seagrass-cargo-artifact-smoke");
const pinocchioSmokeLibPath = resolve(cargoArtifactSmokeRoot, "programs/pinocchio-counter/src/lib.rs");
const nativeSmokeLibPath = resolve(cargoArtifactSmokeRoot, "programs/native-counter/src/lib.rs");
const pinocchioSmokeUri = pathToFileURL(pinocchioSmokeLibPath).href;
const nativeSmokeUri = pathToFileURL(nativeSmokeLibPath).href;
const ecosystemSmokeRoot = resolve(repoRoot, "target/seagrass-ecosystem-smoke");
const ecosystemSmokeLibPath = resolve(ecosystemSmokeRoot, "programs/ecosystem-demo/src/lib.rs");
const ecosystemSmokeUri = pathToFileURL(ecosystemSmokeLibPath).href;

const mainUri = pathToFileURL(resolve(repoRoot, "target/seagrass-smoke.rs")).href;
const splitLibUri = pathToFileURL(resolve(repoRoot, "fixtures/seagrass-split-lib.rs")).href;
const splitAccountsUri = pathToFileURL(
  resolve(repoRoot, "fixtures/seagrass-split-accounts.rs"),
).href;
const splitContextCompletionUri = pathToFileURL(resolve(repoRoot, "target/seagrass-split-context-completion.rs")).href;
const emptySlotCompletionUri = pathToFileURL(resolve(repoRoot, "target/seagrass-empty-slot-completion.rs")).href;
const splitArgsLibUri = pathToFileURL(resolve(repoRoot, "target/seagrass-split-args-lib.rs")).href;
const splitArgsAccountsUri = pathToFileURL(resolve(repoRoot, "target/seagrass-split-args-accounts.rs")).href;
const splitArgsNoInstructionAccountsUri = pathToFileURL(
  resolve(repoRoot, "target/seagrass-split-args-no-instruction-accounts.rs"),
).href;
const missingInstructionArgUri = pathToFileURL(resolve(repoRoot, "target/seagrass-missing-instruction-arg.rs")).href;
const instructionArgOrderUri = pathToFileURL(resolve(repoRoot, "target/seagrass-instruction-arg-order.rs")).href;
const extraInstructionArgUri = pathToFileURL(resolve(repoRoot, "target/seagrass-extra-instruction-arg.rs")).href;
const unknownCtxUri = pathToFileURL(resolve(repoRoot, "target/seagrass-unknown-ctx.rs")).href;
const missingConstraintAccountUri = pathToFileURL(resolve(repoRoot, "target/seagrass-missing-constraint-account.rs")).href;
const missingAccountsUri = pathToFileURL(resolve(repoRoot, "target/seagrass-missing-accounts.rs")).href;
const emptyContextUri = pathToFileURL(resolve(repoRoot, "target/seagrass-empty-context.rs")).href;
const accountsAliasUri = pathToFileURL(resolve(repoRoot, "target/seagrass-accounts-alias.rs")).href;
const foldingUri = pathToFileURL(resolve(repoRoot, "target/seagrass-folding.rs")).href;
const multilineCompletionUri = pathToFileURL(resolve(repoRoot, "target/seagrass-multiline-completion.rs")).href;
const completionGuardrailUri = pathToFileURL(resolve(repoRoot, "target/seagrass-completion-guardrails.rs")).href;
const hoverGuardrailUri = pathToFileURL(resolve(repoRoot, "target/seagrass-hover-guardrails.rs")).href;
const relatedInfoGuardrailUri = pathToFileURL(resolve(repoRoot, "target/seagrass-related-info-guardrails.rs")).href;
const cpiUri = pathToFileURL(resolve(repoRoot, "target/seagrass-cpi.rs")).href;
const proactiveAssistUri = pathToFileURL(resolve(repoRoot, "target/seagrass-proactive-assist.rs")).href;
const proactiveRefreshUri = pathToFileURL(resolve(repoRoot, "target/seagrass-proactive-refresh.rs")).href;
const smokeFixtureWorkspaceUri = pathToFileURL(smokeFixtureRoot).href;

const artifactSmokeSource = `
use anchor_lang::prelude::*;

declare_id!("Artifact1111111111111111111111111111111");

#[program]
pub mod artifact_demo {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
`;

const checkCfgSmokeSource = `
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct CollectFeesV2<'info> {
    pub payer: Signer<'info>,
}
`;

const pinocchioSmokeSource = `
#![no_std]

use pinocchio::{entrypoint, AccountView, Address, ProgramResult};
use pinocchio_pubkey::declare_id;

declare_id!("Pino111111111111111111111111111111111111");

entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Address,
    _accounts: &mut [AccountView],
    _instruction_data: &[u8],
) -> ProgramResult {
    let amount = 10_u64;
    let fee = 1_u64;
    let _net_amount = amount - fee;
    let _tag = _instruction_data[0];
    Ok(())
}
`;

const nativeSmokeSource = `
use solana_program::{
    account_info::AccountInfo,
    declare_id,
    entrypoint,
    entrypoint::ProgramResult,
    instruction::{AccountMeta, Instruction},
    program::invoke,
    pubkey::Pubkey,
};

declare_id!("Native1111111111111111111111111111111111");

entrypoint!(process_instruction);

pub fn process_instruction(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _instruction_data: &[u8],
) -> ProgramResult {
    let bump = 254_u8;
    let _manual_pda = Pubkey::create_program_address(&[b"vault", &[bump]], _program_id)?;
    let account = &_accounts[0];
    let tag = _instruction_data[0];
    let data = account.try_borrow_data()?;
    let _state = State::try_from_slice(&data)?;
    let metas = vec![AccountMeta::new(*account.key, true)];
    let ix = Instruction { program_id: *_program_id, accounts: metas, data: vec![tag] };
    invoke(&ix, _accounts)?;
    Ok(())
}
`;

const ecosystemSmokeSource = `
use anchor_lang::prelude::*;
use anchor_spl::token::{TokenAccount};

declare_id!("Eco1111111111111111111111111111111111111");

#[program]
pub mod ecosystem_demo {
    use super::*;

    pub fn create(_ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(associated_token::mint = token, associated_token::authority = authority)]
    pub vault: Account<'info, TokenAccount>,
    pub token: Account<'info, TokenAccount>,
    pub authority: Signer<'info>,
}
`;

const basicEmptyTutorialSource = `
use anchor_lang::prelude::*;

declare_id!("BasicEmpty111111111111111111111111111111");

#[program]
pub mod basic_empty {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
`;

const basicMutationTutorialSource = `
use anchor_lang::prelude::*;

declare_id!("BasicMutation11111111111111111111111111");

#[program]
pub mod basic_mutation {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, data: u64) -> Result<()> {
        let my_account = &mut ctx.accounts.my_account;
        my_account.data = data;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = payer, space = 8 + MyAccount::INIT_SPACE)]
    pub my_account: Account<'info, MyAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct MyAccount {
    pub data: u64,
}
`;

const basicPdaTutorialSource = `
use anchor_lang::prelude::*;

declare_id!("BasicPda1111111111111111111111111111111");

#[program]
pub mod basic_pda {
    use super::*;

    pub fn update(ctx: Context<Update>, value: u64) -> Result<()> {
        ctx.accounts.state.value = value;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    #[account(mut, seeds = [b"state", authority.key().as_ref()], bump, has_one = authority)]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
}

#[account]
pub struct State {
    pub authority: Pubkey,
    pub value: u64,
}
`;

const puppetMasterCpiSource = `
use anchor_lang::prelude::*;
use puppet::cpi::accounts::SetData;
use puppet::program::Puppet;

declare_id!("PuppetMaster111111111111111111111111111");

#[program]
pub mod puppet_master {
    use super::*;

    pub fn pull_strings(ctx: Context<PullStrings>, data: u64) -> Result<()> {
        let cpi_program = ctx.accounts.puppet_program.to_account_info();
        let cpi_accounts = SetData {
            puppet: ctx.accounts.puppet.to_account_info(),
        };
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        puppet::cpi::set_data(cpi_ctx, data)
    }
}

#[derive(Accounts)]
pub struct PullStrings<'info> {
    #[account(mut)]
    pub puppet: AccountInfo<'info>,
    pub puppet_program: Program<'info, Puppet>,
}
`;

const basicMutationFixture = sourceFixture(
  "basic mutation tutorial",
  "tutorial/basic-mutation/programs/basic-mutation/src/lib.rs",
  basicMutationTutorialSource,
);
const basicPdaFixture = sourceFixture(
  "basic PDA tutorial",
  "tutorial/basic-pda/programs/basic-pda/src/lib.rs",
  basicPdaTutorialSource,
);
const puppetMasterFixture = sourceFixture(
  "puppet-master CPI corpus",
  "corpus/puppet-master/programs/puppet-master/src/lib.rs",
  puppetMasterCpiSource,
);
const anchorTutorialSources = [
  sourceFixture(
    "basic empty tutorial",
    "tutorial/basic-empty/programs/basic-empty/src/lib.rs",
    basicEmptyTutorialSource,
  ),
  basicMutationFixture,
  basicPdaFixture,
];
const anchorCorpusSources = [puppetMasterFixture];

const mainSource = `
use anchor_lang::prelude::*;

#[program]
pub mod smoke {
    use super::*;

    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init)]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}

#[account]
pub struct State {}
`;

const proactiveAssistSource = `
use anchor_lang::prelude::*;

#[program]
pub mod smoke {
    use super::*;

    pub fn update(ctx: Context<UpdateVault>, name: String) -> Result<()> {
        ctx.accounts.vault.count = ctx.accounts.vault.count.checked_add(1).unwrap();
        Ok(())
    }

    pub fn settle_position(ctx: Context<SettlePosition>, position_name: String) -> Result<()> {
        let _token_cpi = CpiContext::new(ctx.accounts.token_program.to_account_info(), ());
        let _metadata_cpi = CpiContext::new(ctx.accounts.metadata_program.to_account_info(), ());
        let _ = position_name;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = payer, space = 8 + Vault::INIT_SPACE)]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub payer: Signer<'info>,
}

#[derive(Accounts)]
pub struct CreateAssociatedToken<'info> {
    #[account(init, payer = payer, associated_token::mint = mint, associated_token::authority = payer)]
    pub token: Account<'info, TokenAccount>,
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UpdateVault<'info> {
    #[account(seeds = [name.as_bytes()])]
    pub vault: Account<'info, Vault>,
}

#[derive(Accounts)]
#[instruction(position_name: String)]
pub struct SettlePosition<'info> {
    #[account(seeds = [b"position", market.key().as_ref(), authority.key().as_ref(), position_name.as_bytes()])]
    pub position: Account<'info, Vault>,
    pub market: AccountInfo<'info>,
    pub authority: Signer<'info>,
    pub token_program: AccountInfo<'info>,
    pub metadata_program: AccountInfo<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct Vault {
    pub count: u64,
}
`;

const proactiveRefreshSource = `
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct RefreshCreate<'info> {
    #[account(init, payer = payer, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub payer: Signer<'info>,
}

#[account]
#[derive(InitSpace)]
pub struct State {
    pub value: u64,
}
`;

const proactiveRefreshUpdatedSource = `
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct RefreshCreate<'info> {
    #[account(init, payer = payer, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[account]
#[derive(InitSpace)]
pub struct State {
    pub value: u64,
}
`;

const emptySlotCompletionSource = `
use anchor_lang::prelude::*;

#[program]
pub mod smoke {
    use super::*;

    pub fn initialize(ctx: Context<Create>) -> Result<()> {
        let state = ctx.accounts.
        Ok(())
    }

    pub fn reset(ctx: Context<) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, 
    pub token_program: Program<'info, 
}

#[account]
pub struct State {}
`;

const splitLibSource = `
use anchor_lang::prelude::*;

pub use instructions::*;

#[program]
pub mod escrow {
    use super::*;

    pub fn make_offer(ctx: Context<MakeOffer>) -> Result<()> {
        let maker = ctx.accounts.maker.key();
        let inner = ctx.accounts.wrapper.inner.key();
        Ok(())
    }
}
`;

const splitAccountsSource = `
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct MakeOffer<'info> {
    pub maker: Signer<'info>,
    pub wrapper: Wrapped<'info>,
}

#[derive(Accounts)]
pub struct Wrapped<'info> {
    pub inner: Account<'info, Inner>,
}
`;

const splitContextCompletionSource = `
use anchor_lang::prelude::*;

pub use instructions::*;

#[program]
pub mod escrow {
    use super::*;

    pub fn make_offer(context: Context<Ma>, id: u64) -> Result<()> {
        Ok(())
    }
}
`;

const splitArgsLibSource = `
use anchor_lang::prelude::*;

pub use instructions::*;

#[program]
pub mod split_args_demo {
    use super::*;

    pub fn create(ctx: Context<CreateMint>, token_decimals: u8) -> Result<()> {
        Ok(())
    }
}
`;

const splitArgsAccountsSource = `
use anchor_lang::prelude::*;

#[derive(Accounts)]
#[instruction(token_decimals: u8)]
pub struct CreateMint<'info> {
    #[account(init, payer = payer, mint::decimals = token_decimals, mint::authority = payer.key())]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
`;

const splitArgsNoInstructionAccountsSource = `
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct CreateMint<'info> {
    #[account(mint::decimals = token_dec)]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
`;

const missingInstructionArgSource = `
use anchor_lang::prelude::*;

#[program]
pub mod missing_instruction_arg_demo {
    use super::*;

    pub fn create(ctx: Context<CreateMint>, _token_decimals: u8) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CreateMint<'info> {
    #[account(init, payer = payer, mint::decimals = _token_decimals, mint::authority = payer.key())]
    pub mint: Account<'info, Mint>,
    pub payer: Signer<'info>,
}
`;

const instructionArgOrderSource = `
use anchor_lang::prelude::*;

#[program]
pub mod instruction_arg_order_demo {
    use super::*;

    pub fn initialize(ctx: Context<Create>, decimals: u8, name: String) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(name: String)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
`;

const extraInstructionArgSource = `
use anchor_lang::prelude::*;

#[program]
pub mod extra_instruction_arg_demo {
    use super::*;

    pub fn initialize(ctx: Context<Create>, decimals: u8) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(decimals: u8, name: String)]
pub struct Create<'info> {
    pub payer: Signer<'info>,
}
`;

const unknownCtxSource = `
use anchor_lang::prelude::*;

#[program]
pub mod typo_demo {
    use super::*;

    pub fn create(ctx: Context<Create>, authority: Pubkey) -> Result<()> {
        let counter = &mut ctx.accounts.acc;
        counter.authority = authority;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + 40)]
    pub counter: Account<'info, Counter>,
    #[account(mut)]
    pub user: Signer<'info>,
    pub system_program: Program<'info, System>,
}
`;

const missingConstraintAccountSource = `
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(init, payer = user, space = 8 + State::INIT_SPACE)]
    pub state: Account<'info, State>,
}
`;

const missingAccountsSource = `
use anchor_lang::prelude::*;

#[program]
pub mod missing_accounts_demo {
    use super::*;

    pub fn missing_offer(ctx: Context<MissingOffer>) -> Result<()> {
        require!(ctx.accounts.maker.is_signer, ErrorCode::MissingSigner);
        let vault = &mut ctx.accounts.vault;
        vault.lamports();
        let maker = ctx.accounts.maker.key();
        let rent = ctx.accounts.rent.key();
        let system = ctx.accounts.system_program.key();
        Ok(())
    }
}
`;

const emptyContextSource = `
use anchor_lang::prelude::*;

#[program]
pub mod empty_context_demo {
    use super::*;

    pub fn fresh_context_demo(context: Context<>, id: u64) -> Result<()> {
        Ok(())
    }
}
`;

const accountsAliasSource = `
use anchor_lang::prelude::*;

#[program]
pub mod accounts_alias_demo {
    use super::*;

    pub fn increment(ctx: Context<Update>) -> Result<()> {
        let accounts = &mut ctx.accounts;
        accounts.counter.count += 1;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub counter: Account<'info, Counter>,
}
`;

const foldingSource = `
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(
        init,
        payer = user,
        space = 8 + State::INIT_SPACE
    )]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
`;

const multilineCompletionSource = `
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Create<'info> {
    #[account(
        init,
        pa
    )]
    pub state: Account<'info, State>,
    pub user: Signer<'info>,
}
`;

const multilineValueCompletionSource = multilineCompletionSource.replace("pa", "payer = u");

const completionGuardrailSource = `
pub struct Context<T> {
    value: T,
}

pub struct Wrapper {
    accounts: AccountsBag,
}

pub struct AccountsBag {
    count: u64,
}

fn helper(ctx: Context<Ma>, wrapper: Wrapper) {
    let _value = ctx.accounts.co;
    let _text = "#[account(pa";
    // #[account(mu
}
`;

const hoverGuardrailSource = `
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Instructions<'info> {
    pub payer: Signer<'info>,
}

fn helper() {
    let _items: Vec<Instructions>;
}
`;

const relatedInfoGuardrailSource = `
use anchor_lang::prelude::*;

#[derive(Accounts)]
pub struct Check<'info> {
    #[account(has_one = missing_authority)]
    pub state: Account<'info, State>,
    pub authority: Signer<'info>,
}

#[account]
pub struct State {
    pub authority: Pubkey,
}
`;

const cpiSource = `
use anchor_lang::prelude::*;
use anchor_lang::solana_program::{instruction::Instruction, program::invoke};

#[derive(Accounts)]
pub struct Cpi<'info> {
    pub source: AccountInfo<'info>,
    pub metadata_program: AccountInfo<'info>,
}

#[program]
pub mod cpi_demo {
    use super::*;

    pub fn cpi(ctx: Context<Cpi>) -> ProgramResult {
        let ix = Instruction {
            program_id: ctx.accounts.metadata_program.key(),
            accounts: vec![],
            data: vec![],
        };
        invoke(&ix, &[ctx.accounts.source.clone(), ctx.accounts.metadata_program.clone()])
    }
}
`;

const [serverCommand, serverArgs] = seagrassServerCommand();
const server = spawn(serverCommand, serverArgs, {
  cwd: repoRoot,
  stdio: ["pipe", "pipe", "inherit"],
});

let serverExited = false;
let serverExitCode: number | null = null;
let serverExitSignal: NodeJS.Signals | null = null;
const serverExit = new Promise<void>((resolveServerExit) => {
  server.once("exit", (code, signal) => {
    serverExited = true;
    serverExitCode = code;
    serverExitSignal = signal;
    resolveServerExit();
  });
});

let nextId = 1;
let buffer = Buffer.alloc(0);
const pending = new Map<number | string, PendingRequest>();
const notifications: LspMessage[] = [];

server.stdout.on("data", (chunk: Buffer) => {
  buffer = Buffer.concat([buffer, chunk]);
  while (true) {
    const headerEnd = buffer.indexOf("\r\n\r\n");
    if (headerEnd === -1) {
      return;
    }

    const header = buffer.slice(0, headerEnd).toString("utf8");
    const lengthMatch = /Content-Length: (\d+)/i.exec(header);
    if (!lengthMatch) {
      throw new Error(`missing Content-Length header: ${header}`);
    }

    const bodyStart = headerEnd + 4;
    const bodyEnd = bodyStart + Number(lengthMatch[1]);
    if (buffer.length < bodyEnd) {
      return;
    }

    const message = JSON.parse(buffer.slice(bodyStart, bodyEnd).toString("utf8")) as LspMessage;
    buffer = buffer.slice(bodyEnd);
    receive(message);
  }
});

function send(message: LspMessage): void {
  const body = JSON.stringify(message);
  server.stdin.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`);
}

function request<T>(method: string, params?: unknown): Promise<T> {
  const id = nextId++;
  const message: LspMessage = { jsonrpc: "2.0", id, method };
  if (params !== undefined) {
    message.params = params;
  }
  send(message);
  return new Promise((resolveResponse, reject) => {
    const timeout = setTimeout(() => {
      pending.delete(id);
      reject(new Error(`timed out waiting for ${method}`));
    }, 15_000);
    pending.set(id, { resolveResponse, reject, timeout });
  });
}

function notify(method: string, params: unknown): void {
  send({ jsonrpc: "2.0", method, params });
}

function receive(message: LspMessage): void {
  if (message.id !== undefined && message.id !== null && pending.has(message.id)) {
    const entry = pending.get(message.id);
    if (!entry) {
      return;
    }
    pending.delete(message.id);
    clearTimeout(entry.timeout);
    if (message.error) {
      entry.reject(new Error(`${message.error.code}: ${message.error.message}`));
    } else {
      entry.resolveResponse(message.result);
    }
    return;
  }

  notifications.push(message);
}

function seagrassServerCommand(): [string, string[]] {
  if (process.env.SEAGRASS_SERVER_BINARY) {
    return [process.env.SEAGRASS_SERVER_BINARY, []];
  }
  if (process.env.SEAGRASS_HOTPATH === "1") {
    return [
      "cargo",
      ["run", "-p", "seagrass", "--features", "hotpath", "--release", "--quiet"],
    ];
  }
  return ["cargo", ["run", "-p", "seagrass", "--quiet"]];
}

async function waitForServerExit(): Promise<void> {
  await Promise.race([
    serverExit,
    new Promise((_, reject) =>
      setTimeout(
        () => reject(new Error("timed out waiting for seagrass to exit")),
        SERVER_EXIT_TIMEOUT_MILLIS,
      ),
    ),
  ]);
  if (serverExitCode !== 0) {
    throw new Error(
      `seagrass exited unexpectedly: code=${serverExitCode} signal=${serverExitSignal}`,
    );
  }
}

function positionAfter(text: string, needle: string): Position {
  const offset = text.indexOf(needle) + needle.length;
  if (offset < needle.length) {
    throw new Error(`needle not found: ${needle}`);
  }
  const prefix = text.slice(0, offset);
  const lines = prefix.split("\n");
  return {
    line: lines.length - 1,
    character: lines.at(-1).length,
  };
}

function positionAt(text: string, needle: string): Position {
  const offset = text.indexOf(needle);
  if (offset < 0) {
    throw new Error(`needle not found: ${needle}`);
  }
  const prefix = text.slice(0, offset);
  const lines = prefix.split("\n");
  return {
    line: lines.length - 1,
    character: lines.at(-1).length,
  };
}

function openDocument(uri: string, text: string): void {
  notify("textDocument/didOpen", {
    textDocument: {
      uri,
      languageId: "rust",
      version: 1,
      text,
    },
  });
}

function assertNoDuplicateDiagnostics(uri: string, diagnostics: DiagnosticReport): void {
  const seen = new Set<string>();
  for (const item of diagnostics.items ?? []) {
    const key = `${item.code}:${item.message}:${item.range.start.line}:${item.range.start.character}:${item.range.end.line}:${item.range.end.character}`;
    if (seen.has(key)) {
      throw new Error(`duplicate diagnostic for ${uri}: ${key}`);
    }
    seen.add(key);
  }
}

async function pullDiagnostics(uri: string): Promise<DiagnosticReport> {
  return await request<DiagnosticReport>("textDocument/diagnostic", {
    textDocument: { uri },
    identifier: "seagrass",
    previousResultId: null,
  });
}

async function sleep(ms: number): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

async function pullDiagnosticsWithRetries(uri: string, attempts = 5): Promise<DiagnosticReport> {
  let report = await pullDiagnostics(uri);
  for (let attempt = 1; attempt < attempts; attempt += 1) {
    if (!(report.items ?? []).some((item) => item.code === "anchor-context-accounts")) {
      return report;
    }
    await sleep(50);
    report = await pullDiagnostics(uri);
  }
  return report;
}

async function requestWithRetries<T>(
  load: () => Promise<T>,
  ready: (value: T) => boolean,
  attempts = 8,
): Promise<T> {
  let value = await load();
  for (let attempt = 1; attempt < attempts; attempt += 1) {
    if (ready(value)) {
      return value;
    }
    await sleep(50);
    value = await load();
  }
  return value;
}

function completionItems(response: CompletionResponse | null | undefined): CompletionItem[] {
  if (!response) {
    return [];
  }
  return Array.isArray(response) ? response : (response.items ?? []);
}

function documentSymbolsInclude(symbols: DocumentSymbol[] | null | undefined, path: string[]): boolean {
  if (!symbols || path.length === 0) {
    return false;
  }
  const [head, ...tail] = path;
  for (const symbol of symbols) {
    if (symbol.name !== head) {
      continue;
    }
    return tail.length === 0 || documentSymbolsInclude(symbol.children, tail);
  }
  return false;
}

function sourceFixture(name: string, relativePath: string, source: string): SmokeFixture {
  const filePath = resolve(smokeFixtureRoot, relativePath);
  mkdirSync(dirname(filePath), { recursive: true });
  writeFileSync(filePath, source);
  return {
    name,
    uri: pathToFileURL(filePath).href,
    source,
  };
}

function writeCheckCfgSmokeFixtures(): void {
  mkdirSync(dirname(anchorDebugSmokeManifestPath), { recursive: true });
  mkdirSync(dirname(anchorDebugSmokeLibPath), { recursive: true });
  mkdirSync(dirname(solanaTargetSmokeManifestPath), { recursive: true });
  mkdirSync(dirname(solanaTargetSmokeLibPath), { recursive: true });
  writeFileSync(
    anchorDebugSmokeManifestPath,
    `
[package]
name = "anchor-debug-check-cfg-smoke"
version = "0.1.0"
edition = "2021"

[features]
default = []

[dependencies]
anchor-lang = "0.32.1"
`,
  );
  writeFileSync(anchorDebugSmokeLibPath, checkCfgSmokeSource);
  writeFileSync(
    solanaTargetSmokeManifestPath,
    `
[package]
name = "solana-target-check-cfg-smoke"
version = "0.1.0"
edition = "2021"

[features]
anchor-debug = []
default = []

[dependencies]
anchor-lang = "0.32.1"
`,
  );
  writeFileSync(solanaTargetSmokeLibPath, checkCfgSmokeSource);
}

function writeArtifactSmokeFixture(): void {
  mkdirSync(dirname(artifactSmokeLibPath), { recursive: true });
  mkdirSync(resolve(artifactSmokeRoot, "target/deploy"), { recursive: true });
  mkdirSync(resolve(artifactSmokeRoot, "target/idl"), { recursive: true });
  writeFileSync(artifactSmokeLibPath, artifactSmokeSource);
  writeFileSync(
    artifactSmokeAnchorTomlPath,
    `
[programs.localnet]
artifact_demo = "Artifact1111111111111111111111111111111"
`,
  );
  writeFileSync(resolve(artifactSmokeRoot, "target/deploy/artifact_demo.so"), "not an elf");
  writeFileSync(
    resolve(artifactSmokeRoot, "target/idl/artifact_demo.json"),
    JSON.stringify({
      address: "Other11111111111111111111111111111111111",
      metadata: { name: "artifact_demo" },
      instructions: [],
    }),
  );
}

function writeCargoArtifactSmokeFixture(): void {
  mkdirSync(dirname(pinocchioSmokeLibPath), { recursive: true });
  mkdirSync(dirname(nativeSmokeLibPath), { recursive: true });
  mkdirSync(resolve(cargoArtifactSmokeRoot, "target/deploy"), { recursive: true });
  writeFileSync(
    resolve(cargoArtifactSmokeRoot, "Cargo.toml"),
    `
[workspace]
members = ["programs/pinocchio-counter", "programs/native-counter"]
resolver = "2"
`,
  );
  writeFileSync(
    resolve(cargoArtifactSmokeRoot, "programs/pinocchio-counter/Cargo.toml"),
    `
[package]
name = "pinocchio-counter"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[dependencies]
pinocchio = { version = "0.11", default-features = false }
pinocchio-pubkey = "0.3"
`,
  );
  writeFileSync(pinocchioSmokeLibPath, pinocchioSmokeSource);
  writeFileSync(resolve(cargoArtifactSmokeRoot, "target/deploy/pinocchio_counter.so"), "not an elf");
  writeFileSync(
    resolve(cargoArtifactSmokeRoot, "programs/native-counter/Cargo.toml"),
    `
[package]
name = "native-counter-package"
version = "0.1.0"
edition = "2021"

[lib]
name = "native_counter_program"
crate-type = ["cdylib", "lib"]

[dependencies]
solana-program = "3"
`,
  );
  writeFileSync(nativeSmokeLibPath, nativeSmokeSource);
  writeFileSync(resolve(cargoArtifactSmokeRoot, "target/deploy/native_counter_program.so"), "not an elf");
}

function writeEcosystemSmokeFixture(): void {
  mkdirSync(dirname(ecosystemSmokeLibPath), { recursive: true });
  writeFileSync(ecosystemSmokeLibPath, ecosystemSmokeSource);
  writeFileSync(
    resolve(ecosystemSmokeRoot, "Anchor.toml"),
    `
[programs.localnet]
ecosystem_demo = "Eco1111111111111111111111111111111111111"
`,
  );
  writeFileSync(
    resolve(ecosystemSmokeRoot, "programs/ecosystem-demo/Cargo.toml"),
    `
[package]
name = "ecosystem-demo"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib", "lib"]

[dependencies]
anchor-lang = "0.31"
anchor-spl = "0.31"
shank = "0.4"

[dev-dependencies]
litesvm = "0.6"
`,
  );
  writeFileSync(
    resolve(ecosystemSmokeRoot, "package.json"),
    JSON.stringify({
      devDependencies: {
        codama: "latest",
        "@solana-program/program-metadata": "latest",
        surfpool: "latest",
      },
      scripts: {
        localnet: "surfpool start",
      },
    }),
  );
  writeFileSync(resolve(ecosystemSmokeRoot, "codama.json"), JSON.stringify({ idl: "idls/missing.codama.json" }));
  writeFileSync(resolve(ecosystemSmokeRoot, "surfpool.toml"), "[surfpool]\n");
}

function assertNoHighSignalFalsePositive(uri: string, diagnostics: DiagnosticReport): void {
  const falsePositiveCodes = new Set([
    "anchor-context-accounts",
    "anchor-missing-account-reference",
    "anchor-missing-init-constraint",
    "anchor-account-usage",
    "anchor-constraint-shape",
  ]);
  const falsePositives = (diagnostics.items ?? []).filter((item) =>
    typeof item.code === "string" && falsePositiveCodes.has(item.code),
  );
  if (falsePositives.length > 0) {
    throw new Error(`Anchor tutorial smoke fixture produced high-signal false positives for ${uri}: ${JSON.stringify(falsePositives)}`);
  }
}

function assertNoCoreSemanticFalsePositive(uri: string, diagnostics: DiagnosticReport): void {
  const falsePositiveCodes = new Set([
    "anchor-context-accounts",
    "anchor-missing-account-reference",
    "anchor-missing-init-constraint",
    "anchor-account-usage",
  ]);
  const falsePositives = (diagnostics.items ?? []).filter((item) =>
    typeof item.code === "string" && falsePositiveCodes.has(item.code),
  );
  if (falsePositives.length > 0) {
    throw new Error(`Anchor corpus smoke fixture produced core semantic false positives for ${uri}: ${JSON.stringify(falsePositives)}`);
  }
}

writeArtifactSmokeFixture();
writeCheckCfgSmokeFixtures();
writeCargoArtifactSmokeFixture();
writeEcosystemSmokeFixture();

try {
  const initializeResult = await request<InitializeResult>("initialize", {
    processId: process.pid,
    rootUri: pathToFileURL(repoRoot).href,
    workspaceFolders: [
      { uri: smokeFixtureWorkspaceUri, name: "smoke-fixtures" },
      { uri: checkCfgSmokeWorkspaceUri, name: "check-cfg-smoke" },
      { uri: pathToFileURL(cargoArtifactSmokeRoot).href, name: "cargo-artifact-smoke" },
    ],
    initializationOptions: {
      seagrass: {
        diagnostics: {
          transport: "pull",
        },
      },
    },
    capabilities: {
      workspace: {
        workspaceFolders: true,
      },
      textDocument: {
        codeAction: { dynamicRegistration: false },
        diagnostic: { dynamicRegistration: false },
      },
    },
  });

  if (!initializeResult?.capabilities?.diagnosticProvider) {
    throw new Error("pull diagnostic provider was not advertised");
  }
  if (!initializeResult?.capabilities?.completionProvider) {
    throw new Error("completion provider was not advertised");
  }
  const completionTriggers = initializeResult.capabilities.completionProvider.triggerCharacters ?? [];
  for (const character of ["s", "S", "_", " ", ".", "<", ",", "="]) {
    if (!completionTriggers.includes(character)) {
      throw new Error(`fast Anchor completion trigger ${JSON.stringify(character)} was not advertised`);
    }
  }
  for (const punctuation of ["#", "[", "(", ":"]) {
    if (completionTriggers.includes(punctuation)) {
      throw new Error(`noisy completion trigger ${JSON.stringify(punctuation)} was advertised`);
    }
  }
  if (!initializeResult?.capabilities?.hoverProvider) {
    throw new Error("hover provider was not advertised");
  }
  const expectedProviders: [keyof NonNullable<InitializeResult["capabilities"]>, string][] = [
    ["documentSymbolProvider", "document symbol provider"],
    ["documentLinkProvider", "document link provider"],
    ["workspaceSymbolProvider", "workspace symbol provider"],
    ["declarationProvider", "declaration provider"],
    ["definitionProvider", "definition provider"],
    ["typeDefinitionProvider", "type definition provider"],
    ["implementationProvider", "implementation provider"],
    ["referencesProvider", "references provider"],
    ["renameProvider", "rename provider"],
    ["codeLensProvider", "code lens provider"],
    ["documentHighlightProvider", "document highlight provider"],
    ["selectionRangeProvider", "selection range provider"],
    ["foldingRangeProvider", "folding range provider"],
    ["signatureHelpProvider", "signature help provider"],
    ["semanticTokensProvider", "semantic tokens provider"],
    ["inlayHintProvider", "inlay hint provider"],
    ["executeCommandProvider", "execute command provider"],
  ];
  for (const [provider, label] of expectedProviders) {
    if (!initializeResult.capabilities?.[provider]) {
      throw new Error(`${label} was not advertised`);
    }
  }
  const semanticTokensProvider = initializeResult.capabilities?.semanticTokensProvider as { range?: boolean } | undefined;
  if (!semanticTokensProvider?.range) {
    throw new Error(`semantic tokens range provider was not advertised: ${JSON.stringify(semanticTokensProvider)}`);
  }
  const commandProvider = initializeResult.capabilities.executeCommandProvider;
  for (const command of [
    "seagrass/status",
    "seagrass/analyze",
    "seagrass/artifacts",
    "seagrass/instructionSummary",
    "seagrass/programReport",
    "seagrass/proposeAssists",
    "seagrass/errorCoverage",
    "seagrass/supportMatrix",
    "seagrass/generatorProfile",
    "seagrass/logs",
    "seagrass/projectCoverage",
  ]) {
    if (!commandProvider?.commands?.includes(command)) {
      throw new Error(`command ${command} was not advertised: ${JSON.stringify(commandProvider)}`);
    }
  }

  notify("initialized", {});

  openDocument(anchorDebugSmokeUri, checkCfgSmokeSource);
  const anchorDebugDiagnostics = await pullDiagnostics(anchorDebugSmokeUri);
  const anchorDebugDiagnostic = anchorDebugDiagnostics.items?.find(
    (item) => item.code === "anchor-check-cfg" && item.data?.quickfix === "add-anchor-debug-feature",
  );
  if (!anchorDebugDiagnostic) {
    throw new Error(`anchor-debug check-cfg diagnostic did not include quickfix data: ${JSON.stringify(anchorDebugDiagnostics)}`);
  }
  const anchorDebugActions = await request<CodeAction[]>("textDocument/codeAction", {
    textDocument: { uri: anchorDebugSmokeUri },
    range: anchorDebugDiagnostic.range,
    context: { diagnostics: [anchorDebugDiagnostic], only: ["quickfix"] },
  });
  const anchorDebugEdit = anchorDebugActions
    ?.find((action) => action.title === "Add `anchor-debug = []` to Cargo.toml features")
    ?.edit?.changes?.[anchorDebugSmokeManifestUri]?.[0]?.newText;
  if (anchorDebugEdit !== "anchor-debug = []\n") {
    throw new Error(`anchor-debug check-cfg quickfix was missing: ${JSON.stringify(anchorDebugActions)}`);
  }

  openDocument(solanaTargetSmokeUri, checkCfgSmokeSource);
  const solanaTargetDiagnostics = await pullDiagnostics(solanaTargetSmokeUri);
  const solanaTargetDiagnostic = solanaTargetDiagnostics.items?.find(
    (item) => item.code === "anchor-check-cfg" && item.data?.quickfix === "add-solana-target-os-check-cfg",
  );
  if (!solanaTargetDiagnostic) {
    throw new Error(`Solana target_os check-cfg diagnostic did not include quickfix data: ${JSON.stringify(solanaTargetDiagnostics)}`);
  }
  const solanaTargetActions = await request<CodeAction[]>("textDocument/codeAction", {
    textDocument: { uri: solanaTargetSmokeUri },
    range: solanaTargetDiagnostic.range,
    context: { diagnostics: [solanaTargetDiagnostic], only: ["quickfix"] },
  });
  const solanaTargetEdit = solanaTargetActions
    ?.find((action) => action.title === "Allow `target_os = \"solana\"` in Cargo check-cfg")
    ?.edit?.changes?.[solanaTargetSmokeManifestUri]?.[0]?.newText;
  if (
    !solanaTargetEdit?.includes("[package.lints.rust.unexpected_cfgs]") ||
    !solanaTargetEdit.includes("'cfg(target_os, values(\"solana\"))'")
  ) {
    throw new Error(`Solana target_os check-cfg quickfix was missing: ${JSON.stringify(solanaTargetActions)}`);
  }

  if (anchorTutorialSources.length < MIN_ANCHOR_TUTORIAL_SMOKE_FIXTURES) {
    throw new Error(`expected Anchor tutorial smoke coverage, found ${anchorTutorialSources.length} files`);
  }
  for (const tutorial of anchorTutorialSources) {
    openDocument(tutorial.uri, tutorial.source);
    const diagnostics = await pullDiagnostics(tutorial.uri);
    assertNoDuplicateDiagnostics(tutorial.uri, diagnostics);
    assertNoHighSignalFalsePositive(tutorial.uri, diagnostics);
  }
  for (const corpusFile of anchorCorpusSources) {
    openDocument(corpusFile.uri, corpusFile.source);
    const diagnostics = await pullDiagnostics(corpusFile.uri);
    assertNoDuplicateDiagnostics(corpusFile.uri, diagnostics);
    assertNoCoreSemanticFalsePositive(corpusFile.uri, diagnostics);
  }

  openDocument(basicMutationFixture.uri, basicMutationFixture.source);
  const basicMutationDiagnostics = await pullDiagnostics(basicMutationFixture.uri);
  assertNoDuplicateDiagnostics(basicMutationFixture.uri, basicMutationDiagnostics);
  if (
    basicMutationDiagnostics.items?.some(
      (item) => item.code === "anchor-missing-account-reference" || item.code === "anchor-missing-init-constraint",
    )
  ) {
    throw new Error(`${basicMutationFixture.name} produced false Anchor diagnostics: ${JSON.stringify(basicMutationDiagnostics.items)}`);
  }
  const basicMutationHover = await request<Hover>("textDocument/hover", {
    textDocument: { uri: basicMutationFixture.uri },
    position: positionAt(basicMutationFixture.source, "my_account.data = data"),
  });
  if (
    !basicMutationHover?.contents?.value?.includes("Anchor field in `Initialize`") ||
    !basicMutationHover.contents.value.includes("Used by: `initialize` mutates")
  ) {
    throw new Error(`${basicMutationFixture.name} local binding hover was not Anchor-aware: ${JSON.stringify(basicMutationHover)}`);
  }
  const basicMutationDefinition = await request<Location[] | Location | null>("textDocument/definition", {
    textDocument: { uri: basicMutationFixture.uri },
    position: positionAfter(basicMutationFixture.source, "Context<Initialize"),
  });
  const basicMutationDefinitions = Array.isArray(basicMutationDefinition)
    ? basicMutationDefinition
    : basicMutationDefinition
      ? [basicMutationDefinition]
      : [];
  const basicMutationAccountsLine = positionAt(basicMutationFixture.source, "pub struct Initialize").line;
  if (
    !basicMutationDefinitions.some(
      (location) => location.uri === basicMutationFixture.uri && location.range.start.line === basicMutationAccountsLine,
    )
  ) {
    throw new Error(`${basicMutationFixture.name} Context<Initialize> definition did not jump to the accounts struct: ${JSON.stringify(basicMutationDefinition)}`);
  }

  openDocument(basicPdaFixture.uri, basicPdaFixture.source);
  const basicPdaDiagnostics = await pullDiagnostics(basicPdaFixture.uri);
  assertNoDuplicateDiagnostics(basicPdaFixture.uri, basicPdaDiagnostics);
  if (basicPdaDiagnostics.items?.some((item) => item.code === "anchor-constraint-shape")) {
    throw new Error(`${basicPdaFixture.name} valid PDA/has_one constraints were incorrectly flagged: ${JSON.stringify(basicPdaDiagnostics.items)}`);
  }
  const basicPdaHover = await request<Hover>("textDocument/hover", {
    textDocument: { uri: basicPdaFixture.uri },
    position: positionAt(basicPdaFixture.source, "seeds = ["),
  });
  if (!basicPdaHover?.contents?.value?.includes("seeds")) {
    throw new Error(`${basicPdaFixture.name} PDA hover did not explain seeds: ${JSON.stringify(basicPdaHover)}`);
  }

  openDocument(puppetMasterFixture.uri, puppetMasterFixture.source);
  const puppetMasterDiagnostics = await pullDiagnostics(puppetMasterFixture.uri);
  assertNoDuplicateDiagnostics(puppetMasterFixture.uri, puppetMasterDiagnostics);
  if (puppetMasterDiagnostics.items?.some((item) => item.code === "anchor-missing-account-reference")) {
    throw new Error(`${puppetMasterFixture.name} CPI account usage was incorrectly flagged: ${JSON.stringify(puppetMasterDiagnostics.items)}`);
  }

  openDocument(mainUri, mainSource);
  const mainDiagnostics = await pullDiagnostics(mainUri);
  const mainSymbols = await request<DocumentSymbol[]>("textDocument/documentSymbol", {
    textDocument: { uri: mainUri },
  });
  if (!documentSymbolsInclude(mainSymbols, ["Create", "state", "init"])) {
    throw new Error(`document symbols did not expose account constraints: ${JSON.stringify(mainSymbols)}`);
  }
  const workspaceSymbols = await request<SymbolInformation[]>("workspace/symbol", {
    query: "Create",
  });
  if (!workspaceSymbols?.some((symbol) => symbol.name === "Create" && symbol.location?.uri === mainUri)) {
    throw new Error(`workspace/symbol did not include Anchor account structs: ${JSON.stringify(workspaceSymbols)}`);
  }
  const mainHighlights = await request<DocumentHighlight[]>("textDocument/documentHighlight", {
    textDocument: { uri: mainUri },
    position: positionAfter(mainSource, "Context<Create"),
  });
  if (!mainHighlights?.length) {
    throw new Error(`document highlights were empty for Anchor context type: ${JSON.stringify(mainHighlights)}`);
  }
  const mainReferences = await request<Location[]>("textDocument/references", {
    textDocument: { uri: mainUri },
    position: positionAfter(mainSource, "Context<Create"),
    context: { includeDeclaration: true },
  });
  if (!mainReferences?.some((location) => location.uri === mainUri && location.range.start.line >= 11)) {
    throw new Error(`references did not include Anchor account struct declaration: ${JSON.stringify(mainReferences)}`);
  }
  const mainDeclaration = await request<Location[] | Location | null>("textDocument/declaration", {
    textDocument: { uri: mainUri },
    position: positionAfter(mainSource, "Context<Create"),
  });
  const normalizedMainDeclarations = Array.isArray(mainDeclaration)
    ? mainDeclaration
    : mainDeclaration
      ? [mainDeclaration]
      : [];
  if (!normalizedMainDeclarations.some((location) => location.uri === mainUri && location.range.start.line >= 11)) {
    throw new Error(`declaration did not include local Anchor account struct: ${JSON.stringify(mainDeclaration)}`);
  }
  const mainTypeDefinitions = await request<Location[] | Location | null>("textDocument/typeDefinition", {
    textDocument: { uri: mainUri },
    position: positionAfter(mainSource, "state: Account<'info, State"),
  });
  const normalizedMainTypeDefinitions = Array.isArray(mainTypeDefinitions)
    ? mainTypeDefinitions
    : mainTypeDefinitions
      ? [mainTypeDefinitions]
      : [];
  if (!normalizedMainTypeDefinitions.some((location) => location.uri === mainUri && location.range.start.line >= 17)) {
    throw new Error(`typeDefinition did not include local Anchor account data struct: ${JSON.stringify(mainTypeDefinitions)}`);
  }
  openDocument(emptySlotCompletionUri, emptySlotCompletionSource);
  const emptyContextCompletion = await request<CompletionResponse>("textDocument/completion", {
    textDocument: { uri: emptySlotCompletionUri },
    position: positionAfter(emptySlotCompletionSource, "reset(ctx: Context<"),
    context: { triggerKind: 2, triggerCharacter: "<" },
  });
  if (!completionItems(emptyContextCompletion).some((item) => item.label === "Create")) {
    throw new Error(`empty Context< completion did not include Create: ${JSON.stringify(emptyContextCompletion)}`);
  }
  const dotAccountCompletion = await request<CompletionResponse>("textDocument/completion", {
    textDocument: { uri: emptySlotCompletionUri },
    position: positionAfter(emptySlotCompletionSource, "ctx.accounts."),
    context: { triggerKind: 2, triggerCharacter: "." },
  });
  if (!completionItems(dotAccountCompletion).some((item) => item.label === "state")) {
    throw new Error(`ctx.accounts. completion did not include state: ${JSON.stringify(dotAccountCompletion)}`);
  }
  const emptyGenericCompletion = await request<CompletionResponse>("textDocument/completion", {
    textDocument: { uri: emptySlotCompletionUri },
    position: positionAfter(emptySlotCompletionSource, "Sysvar<'info, "),
    context: { triggerKind: 2, triggerCharacter: " " },
  });
  if (!completionItems(emptyGenericCompletion).some((item) => item.label === "Rent")) {
    throw new Error(`empty Sysvar generic completion did not include Rent: ${JSON.stringify(emptyGenericCompletion)}`);
  }
  const emptyProgramGenericCompletion = await request<CompletionResponse>("textDocument/completion", {
    textDocument: { uri: emptySlotCompletionUri },
    position: positionAfter(emptySlotCompletionSource, "Program<'info, "),
    context: { triggerKind: 2, triggerCharacter: " " },
  });
  if (!completionItems(emptyProgramGenericCompletion).some((item) => item.label === "System")) {
    throw new Error(`empty Program generic completion did not include System: ${JSON.stringify(emptyProgramGenericCompletion)}`);
  }
  const mainImplementations = await request<Location[] | Location | null>("textDocument/implementation", {
    textDocument: { uri: mainUri },
    position: positionAfter(mainSource, "pub struct Create"),
  });
  const normalizedMainImplementations = Array.isArray(mainImplementations)
    ? mainImplementations
    : mainImplementations
      ? [mainImplementations]
      : [];
  if (!normalizedMainImplementations.some((location) => location.uri === mainUri && location.range.start.line >= 5 && location.range.start.line <= 8)) {
    throw new Error(`implementation did not jump from Anchor accounts struct to program instruction: ${JSON.stringify(mainImplementations)}`);
  }
  const mainCodeLens = await request<CodeLens[]>("textDocument/codeLens", {
    textDocument: { uri: mainUri },
  });
  if (
    !mainCodeLens?.some(
      (lens) =>
        lens.command?.title === "1 Anchor instruction" &&
        lens.command.command === "seagrass/analyze" &&
        (lens.command.arguments?.[0] as JsonObject | undefined)?.context === "Create" &&
        lens.data?.context === "Create",
    )
  ) {
    throw new Error(`codeLens did not expose Anchor instruction count for accounts context: ${JSON.stringify(mainCodeLens)}`);
  }
  if (
    !mainCodeLens.some(
      (lens) =>
        lens.command?.title === "Context<Create>" &&
        lens.command.command === "seagrass/analyze" &&
        lens.data?.kind === "anchor.programInstruction" &&
        lens.data?.instruction === "initialize" &&
        lens.data?.context === "Create",
    )
  ) {
    throw new Error(`codeLens did not expose Anchor context for program instruction: ${JSON.stringify(mainCodeLens)}`);
  }
  const mainSemanticTokens = await request<SemanticTokens>("textDocument/semanticTokens/full", {
    textDocument: { uri: mainUri },
  });
  if (!mainSemanticTokens?.data?.length) {
    throw new Error(`semantic tokens were empty for Anchor document: ${JSON.stringify(mainSemanticTokens)}`);
  }
  const mainSemanticTokensRange = await request<SemanticTokens>("textDocument/semanticTokens/range", {
    textDocument: { uri: mainUri },
    range: {
      start: { line: 11, character: 0 },
      end: { line: 16, character: 1 },
    },
  });
  if (!mainSemanticTokensRange?.data?.length) {
    throw new Error(`semantic token range result was empty for Anchor accounts struct: ${JSON.stringify(mainSemanticTokensRange)}`);
  }
  const mainSelectionRanges = await request<SelectionRange[]>("textDocument/selectionRange", {
    textDocument: { uri: mainUri },
    positions: [positionAfter(mainSource, "#[account(in")],
  });
  const constraintSelection = mainSelectionRanges[0];
  if (
    !constraintSelection?.parent?.parent ||
    constraintSelection.range.start.line !== constraintSelection.parent.range.start.line ||
    constraintSelection.parent.range.end.line >= constraintSelection.parent.parent.range.end.line
  ) {
    throw new Error(`selection range did not expand through Anchor constraint hierarchy: ${JSON.stringify(mainSelectionRanges)}`);
  }
  openDocument(foldingUri, foldingSource);
  const foldingRanges = await request<FoldingRange[]>("textDocument/foldingRange", {
    textDocument: { uri: foldingUri },
  });
  if (
    !foldingRanges?.some(
      (range) =>
        range.collapsedText === "#[account(...)]" &&
        typeof range.startLine === "number" &&
        typeof range.endLine === "number" &&
        range.endLine > range.startLine,
    )
  ) {
    throw new Error(`folding ranges did not include multiline account constraint: ${JSON.stringify(foldingRanges)}`);
  }
  openDocument(multilineCompletionUri, multilineCompletionSource);
  const multilineCompletion = await request<CompletionResponse>("textDocument/completion", {
    textDocument: { uri: multilineCompletionUri },
    position: positionAfter(multilineCompletionSource, "        pa"),
  });
  const multilineItems = completionItems(multilineCompletion);
  const payerCompletion = multilineItems.find((item) => item.label === "payer =");
  if (!payerCompletion) {
    throw new Error(`multiline account constraint completion did not include payer: ${JSON.stringify(multilineCompletion)}`);
  }
  notify("textDocument/didChange", {
    textDocument: { uri: multilineCompletionUri, version: 2 },
    contentChanges: [{ text: multilineValueCompletionSource }],
  });
  const multilineValueCompletion = await request<CompletionResponse>("textDocument/completion", {
    textDocument: { uri: multilineCompletionUri },
    position: positionAfter(multilineValueCompletionSource, "payer = u"),
  });
  if (!completionItems(multilineValueCompletion).some((item) => item.label === "user")) {
    throw new Error(`multiline account constraint value completion did not include signer: ${JSON.stringify(multilineValueCompletion)}`);
  }
  openDocument(completionGuardrailUri, completionGuardrailSource);
  for (const [cursor, triggerCharacter] of [
    ["Context<Ma", "<"],
    ["ctx.accounts.co", "."],
    ["\"#[account(pa", "a"],
    ["// #[account(mu", "u"],
  ] as const) {
    const guardrailCompletion = await request<CompletionResponse | null>("textDocument/completion", {
      textDocument: { uri: completionGuardrailUri },
      position: positionAfter(completionGuardrailSource, cursor),
      context: { triggerKind: 2, triggerCharacter },
    });
    const items = completionItems(guardrailCompletion);
    if (items.length > 0) {
      throw new Error(`non-Anchor completion guardrail returned items at ${cursor}: ${JSON.stringify(items)}`);
    }
  }
  openDocument(hoverGuardrailUri, hoverGuardrailSource);
  const shadowedContextHover = await request<Hover | null>("textDocument/hover", {
    textDocument: { uri: hoverGuardrailUri },
    position: positionAfter(hoverGuardrailSource, "Vec<"),
  });
  if (shadowedContextHover?.contents?.value?.includes("Anchor accounts context")) {
    throw new Error(`hover treated Vec<Instructions> as an Anchor context: ${JSON.stringify(shadowedContextHover)}`);
  }
  openDocument(relatedInfoGuardrailUri, relatedInfoGuardrailSource);
  const relatedInfoDiagnostics = await pullDiagnostics(relatedInfoGuardrailUri);
  const hasOneDiagnostic = relatedInfoDiagnostics.items?.find(
    (item) =>
      item.code === "anchor-constraint-shape" &&
      item.message?.includes("has_one") &&
      item.message?.includes("missing_authority"),
  );
  if (!hasOneDiagnostic) {
    throw new Error(`has_one related-info guardrail diagnostic was missing: ${JSON.stringify(relatedInfoDiagnostics.items)}`);
  }
  if (
    !hasOneDiagnostic.relatedInformation?.some((info) => info.message?.includes("declares candidate field `authority`"))
  ) {
    throw new Error(`has_one diagnostic lacked candidate-field related info: ${JSON.stringify(hasOneDiagnostic)}`);
  }
  const foldingLinks = await request<DocumentLink[]>("textDocument/documentLink", {
    textDocument: { uri: foldingUri },
  });
  if (!foldingLinks?.length) {
    throw new Error(`document link endpoint returned no Anchor links: ${JSON.stringify(foldingLinks)}`);
  }
  const foldingSignatureHelp = await request<SignatureHelp>("textDocument/signatureHelp", {
    textDocument: { uri: foldingUri },
    position: positionAfter(foldingSource, "space"),
  });
  if (!foldingSignatureHelp?.signatures?.length) {
    throw new Error(`signature help endpoint returned no Anchor signatures: ${JSON.stringify(foldingSignatureHelp)}`);
  }
  const initDiagnostic = mainDiagnostics?.items?.find(
    (item) => item.code === "anchor-init-constraints" && item.data?.quickfix === "init-placeholders",
  );
  if (!initDiagnostic) {
    throw new Error(`missing init diagnostic with quickfix data: ${JSON.stringify(mainDiagnostics)}`);
  }
  const initActions = await request<CodeAction[]>("textDocument/codeAction", {
    textDocument: { uri: mainUri },
    range: initDiagnostic.range,
    context: { diagnostics: [initDiagnostic], only: ["quickfix"] },
  });
  const firstInitAction = initActions[0];
  if (!firstInitAction) {
    throw new Error("init quickfix action was not returned");
  }
  const resolvedInit = await request<CodeAction>("codeAction/resolve", firstInitAction);
  const initEditText = resolvedInit?.edit?.changes?.[mainUri]?.[0]?.newText;
  if (!initEditText) {
    throw new Error(`init quickfix resolve did not return an edit: ${JSON.stringify(resolvedInit)}`);
  }

  openDocument(splitAccountsUri, splitAccountsSource);
  openDocument(splitLibUri, splitLibSource);
  const splitDiagnostics = await pullDiagnosticsWithRetries(splitLibUri);
  if (splitDiagnostics?.items?.some((item) => item.code === "anchor-context-accounts")) {
    throw new Error(`split workspace context was incorrectly flagged: ${JSON.stringify(splitDiagnostics.items)}`);
  }
  const splitHover = await requestWithRetries(
    () =>
      request<Hover>("textDocument/hover", {
        textDocument: { uri: splitLibUri },
        position: positionAfter(splitLibSource, "Context<MakeOffer"),
      }),
    (hover) => Boolean(hover?.contents?.value?.includes("`maker`: `Signer`")),
  );
  if (!splitHover?.contents?.value?.includes("`maker`: `Signer`")) {
    throw new Error(`split workspace hover did not include account fields: ${JSON.stringify(splitHover)}`);
  }
  const splitContextAnalysis = await requestWithRetries(
    () =>
      request<AnalysisReport>("workspace/executeCommand", {
        command: "seagrass/analyze",
        arguments: [{ uri: splitAccountsUri, context: "MakeOffer" }],
      }),
    (analysis) =>
      Array.isArray(analysis?.focus?.instructions) &&
      analysis.focus.instructions.some(
        (instruction) =>
          (instruction as JsonObject).name === "make_offer" &&
          (instruction as JsonObject).uri === splitLibUri,
      ),
  );
  if (
    splitContextAnalysis?.focus?.kind !== "anchor.accountsContext" ||
    splitContextAnalysis.focus?.context !== "MakeOffer" ||
    !Array.isArray(splitContextAnalysis.focus?.instructions) ||
    !splitContextAnalysis.focus.instructions.some(
      (instruction) =>
        (instruction as JsonObject).name === "make_offer" &&
        (instruction as JsonObject).uri === splitLibUri,
    )
  ) {
    throw new Error(`split accounts context analysis did not include workspace handler: ${JSON.stringify(splitContextAnalysis)}`);
  }
  const splitInstructionAnalysis = await request<AnalysisReport>("workspace/executeCommand", {
    command: "seagrass/analyze",
    arguments: [{ uri: splitLibUri, instruction: "make_offer" }],
  });
  if (
    splitInstructionAnalysis?.focus?.kind !== "anchor.programInstruction" ||
    splitInstructionAnalysis.focus?.instruction !== "make_offer" ||
    !Array.isArray(splitInstructionAnalysis.focus?.contextFields) ||
    !splitInstructionAnalysis.focus.contextFields.some(
      (field) =>
        (field as JsonObject).name === "maker" &&
        (field as JsonObject).type === "Signer" &&
        (field as JsonObject).source === "workspaceIndex",
    )
  ) {
    throw new Error(`split instruction analysis did not include workspace context fields: ${JSON.stringify(splitInstructionAnalysis)}`);
  }
  if (
    !Array.isArray(splitInstructionAnalysis.focus?.resolvedAccountUsages) ||
    !splitInstructionAnalysis.focus.resolvedAccountUsages.some(
      (usage) =>
        (usage as JsonObject).name === "maker" &&
        (usage as JsonObject).declared === true &&
        (usage as JsonObject).type === "Signer" &&
        (usage as JsonObject).source === "workspaceIndex",
    )
  ) {
    throw new Error(`split instruction analysis did not resolve workspace account usage: ${JSON.stringify(splitInstructionAnalysis)}`);
  }
  if (
    !Array.isArray(splitInstructionAnalysis.focus?.resolvedAccountPathUsages) ||
    !splitInstructionAnalysis.focus.resolvedAccountPathUsages.some((usage) => {
      const path = usage as JsonObject;
      return path.path === "wrapper.inner" &&
        Array.isArray(path.segments) &&
        path.segments.some((segment) => {
          const item = segment as JsonObject;
          return item.name === "inner" &&
            item.declared === true &&
            item.container === "Wrapped" &&
            item.type === "Account<Inner>" &&
            item.source === "workspaceIndex";
        });
    })
  ) {
    throw new Error(`split instruction analysis did not resolve workspace account path usage: ${JSON.stringify(splitInstructionAnalysis)}`);
  }
  openDocument(splitContextCompletionUri, splitContextCompletionSource);
  const contextCompletion = await request<CompletionResponse>("textDocument/completion", {
    textDocument: { uri: splitContextCompletionUri },
    position: positionAfter(splitContextCompletionSource, "Context<Ma"),
  });
  const contextItems = completionItems(contextCompletion);
  if (!contextItems.some((item) => item.label === "MakeOffer")) {
    throw new Error(`Context<Ma> completion did not include MakeOffer: ${JSON.stringify(contextCompletion)}`);
  }

  openDocument(splitArgsAccountsUri, splitArgsAccountsSource);
  openDocument(splitArgsLibUri, splitArgsLibSource);
  openDocument(splitArgsNoInstructionAccountsUri, splitArgsNoInstructionAccountsSource);
  const splitArgsInstructionContextCompletion = await requestWithRetries(
    () =>
      request<CompletionResponse>("textDocument/completion", {
        textDocument: { uri: splitArgsNoInstructionAccountsUri },
        position: positionAfter(splitArgsNoInstructionAccountsSource, "mint::decimals = token_dec"),
      }),
    (completion) => completionItems(completion).some((item) => item.label === "token_decimals"),
  );
  if (!completionItems(splitArgsInstructionContextCompletion).some((item) => item.label === "token_decimals")) {
    throw new Error(`account constraint completion did not use linked instruction argument: ${JSON.stringify(splitArgsInstructionContextCompletion)}`);
  }
  const splitArgsDiagnostics = await pullDiagnostics(splitArgsAccountsUri);
  if (splitArgsDiagnostics?.items?.some((item) => item.code === "anchor-missing-instruction-argument")) {
    throw new Error(`split instruction argument was not resolved: ${JSON.stringify(splitArgsDiagnostics.items)}`);
  }
  const splitArgsPrepareRename = await request<PrepareRename>("textDocument/prepareRename", {
    textDocument: { uri: splitArgsLibUri },
    position: positionAt(splitArgsLibSource, "token_decimals: u8"),
  });
  if (splitArgsPrepareRename?.placeholder !== "token_decimals") {
    throw new Error(`prepareRename did not identify Anchor instruction argument: ${JSON.stringify(splitArgsPrepareRename)}`);
  }
  const splitArgsDefinition = await request<Location[] | Location | null>("textDocument/definition", {
    textDocument: { uri: splitArgsAccountsUri },
    position: positionAt(splitArgsAccountsSource, "token_decimals"),
  });
  const splitArgsDefinitions = Array.isArray(splitArgsDefinition)
    ? splitArgsDefinition
    : splitArgsDefinition
      ? [splitArgsDefinition]
      : [];
  if (!splitArgsDefinitions.some((location) => location.uri === splitArgsLibUri && location.range.start.line === 9)) {
    throw new Error(`definition did not jump from split Anchor constraint to handler argument: ${JSON.stringify(splitArgsDefinition)}`);
  }
  const splitArgsReferences = await request<Location[]>("textDocument/references", {
    textDocument: { uri: splitArgsLibUri },
    position: positionAt(splitArgsLibSource, "token_decimals: u8"),
    context: { includeDeclaration: true },
  });
  if (
    !splitArgsReferences?.some((location) => location.uri === splitArgsLibUri && location.range.start.line === 9) ||
    !splitArgsReferences?.some((location) => location.uri === splitArgsAccountsUri && location.range.start.line === 4) ||
    !splitArgsReferences?.some((location) => location.uri === splitArgsAccountsUri && location.range.start.line === 6)
  ) {
    throw new Error(`references did not include split Anchor instruction argument constraint use: ${JSON.stringify(splitArgsReferences)}`);
  }
  const splitArgsHover = await request<Hover>("textDocument/hover", {
    textDocument: { uri: splitArgsAccountsUri },
    position: positionAt(splitArgsAccountsSource, "token_decimals"),
  });
  if (
    !splitArgsHover?.contents?.value?.includes("Anchor instruction argument for `Context<CreateMint>`") ||
    !splitArgsHover.contents.value.includes("mint::decimals")
  ) {
    throw new Error(`hover did not explain split Anchor instruction argument constraint use: ${JSON.stringify(splitArgsHover)}`);
  }
  const splitArgsRename = await request<WorkspaceEdit>("textDocument/rename", {
    textDocument: { uri: splitArgsLibUri },
    position: positionAt(splitArgsLibSource, "token_decimals: u8"),
    newName: "mint_decimals",
  });
  const splitArgsLibRenameEdits = splitArgsRename?.changes?.[splitArgsLibUri] ?? [];
  const splitArgsAccountsRenameEdits = splitArgsRename?.changes?.[splitArgsAccountsUri] ?? [];
  if (
    !splitArgsLibRenameEdits.some((edit) => edit.newText === "mint_decimals" && edit.range.start.line === 9) ||
    !splitArgsAccountsRenameEdits.some((edit) => edit.newText === "mint_decimals" && edit.range.start.line === 4) ||
    !splitArgsAccountsRenameEdits.some((edit) => edit.newText === "mint_decimals" && edit.range.start.line === 6)
  ) {
    throw new Error(`rename did not edit split Anchor instruction argument references: ${JSON.stringify(splitArgsRename)}`);
  }

  openDocument(missingInstructionArgUri, missingInstructionArgSource);
  const missingInstructionArgDiagnostics = await pullDiagnostics(missingInstructionArgUri);
  const missingInstructionArgDiagnostic = missingInstructionArgDiagnostics?.items?.find(
    (item) =>
      item.code === "anchor-missing-instruction-argument" &&
      item.data?.quickfix === "add-instruction-argument" &&
      item.data?.argument === "_token_decimals",
  );
  if (!missingInstructionArgDiagnostic) {
    throw new Error(`missing instruction argument diagnostic did not include quickfix data: ${JSON.stringify(missingInstructionArgDiagnostics)}`);
  }
  if (
    !missingInstructionArgDiagnostic.relatedInformation?.some((info) =>
      info.message?.includes("handler `create` declares `_token_decimals`"),
    )
  ) {
    throw new Error(`missing instruction argument diagnostic did not include handler related information: ${JSON.stringify(missingInstructionArgDiagnostic)}`);
  }
  const missingInstructionArgActions = await request<CodeAction[]>("textDocument/codeAction", {
    textDocument: { uri: missingInstructionArgUri },
    range: missingInstructionArgDiagnostic.range,
    context: { diagnostics: [missingInstructionArgDiagnostic], only: ["quickfix"] },
  });
  const missingInstructionArgEdit = missingInstructionArgActions
    ?.find((action) => action.title?.includes("_token_decimals: u8"))
    ?.edit?.changes?.[missingInstructionArgUri]?.[0]?.newText;
  if (missingInstructionArgEdit !== "#[instruction(_token_decimals: u8)]\n") {
    throw new Error(`missing instruction argument quickfix was missing: ${JSON.stringify(missingInstructionArgActions)}`);
  }

  openDocument(instructionArgOrderUri, instructionArgOrderSource);
  const instructionArgOrderDiagnostics = await pullDiagnostics(instructionArgOrderUri);
  const instructionArgOrderDiagnostic = instructionArgOrderDiagnostics?.items?.find(
    (item) =>
      item.code === "anchor-missing-instruction-argument" &&
      item.data?.quickfix === "replace-instruction-argument" &&
      item.data?.reason === "instruction-attribute-order" &&
      item.data?.expected === "decimals",
  );
  if (!instructionArgOrderDiagnostic) {
    throw new Error(`instruction argument order diagnostic did not include replacement quickfix data: ${JSON.stringify(instructionArgOrderDiagnostics)}`);
  }
  if (
    !instructionArgOrderDiagnostic.relatedInformation?.some((info) =>
      info.message?.includes("expects `decimals`"),
    )
  ) {
    throw new Error(`instruction argument order diagnostic did not include handler related information: ${JSON.stringify(instructionArgOrderDiagnostic)}`);
  }
  const instructionArgOrderActions = await request<CodeAction[]>("textDocument/codeAction", {
    textDocument: { uri: instructionArgOrderUri },
    range: instructionArgOrderDiagnostic.range,
    context: { diagnostics: [instructionArgOrderDiagnostic], only: ["quickfix"] },
  });
  const instructionArgOrderEdit = instructionArgOrderActions
    ?.find((action) => action.title?.includes("decimals: u8"))
    ?.edit?.changes?.[instructionArgOrderUri]?.[0]?.newText;
  if (instructionArgOrderEdit !== "decimals: u8") {
    throw new Error(`instruction argument order replacement quickfix was missing: ${JSON.stringify(instructionArgOrderActions)}`);
  }

  openDocument(extraInstructionArgUri, extraInstructionArgSource);
  const extraInstructionArgDiagnostics = await pullDiagnostics(extraInstructionArgUri);
  const extraInstructionArgDiagnostic = extraInstructionArgDiagnostics?.items?.find(
    (item) =>
      item.code === "anchor-missing-instruction-argument" &&
      item.data?.quickfix === "remove-instruction-argument" &&
      item.data?.reason === "extra-instruction-attribute-argument" &&
      item.data?.argument === "name",
  );
  if (!extraInstructionArgDiagnostic) {
    throw new Error(`extra instruction argument diagnostic did not include remove quickfix data: ${JSON.stringify(extraInstructionArgDiagnostics)}`);
  }
  const extraInstructionArgActions = await request<CodeAction[]>("textDocument/codeAction", {
    textDocument: { uri: extraInstructionArgUri },
    range: extraInstructionArgDiagnostic.range,
    context: { diagnostics: [extraInstructionArgDiagnostic], only: ["quickfix"] },
  });
  const extraInstructionArgEdit = extraInstructionArgActions
    ?.find((action) => action.title === "Remove `name` from #[instruction]")
    ?.edit?.changes?.[extraInstructionArgUri]?.[0]?.newText;
  if (extraInstructionArgEdit !== "") {
    throw new Error(`extra instruction argument remove quickfix was missing: ${JSON.stringify(extraInstructionArgActions)}`);
  }

  openDocument(unknownCtxUri, unknownCtxSource);
  const unknownDiagnostics = await pullDiagnostics(unknownCtxUri);
  const unknownDiagnostic = unknownDiagnostics?.items?.find(
    (item) =>
      item.code === "anchor-missing-account-reference" &&
      item.data?.reason === "unknown-ctx-account-field" &&
      item.data?.account === "acc",
  );
  if (!unknownDiagnostic) {
    throw new Error(`unknown ctx.accounts field was not diagnosed: ${JSON.stringify(unknownDiagnostics)}`);
  }
  const unknownActions = await request<CodeAction[]>("textDocument/codeAction", {
    textDocument: { uri: unknownCtxUri },
    range: unknownDiagnostic.range,
    context: { diagnostics: [unknownDiagnostic], only: ["quickfix"] },
  });
  if (!unknownActions?.some((action) => action.title === "Replace `acc` with `counter`")) {
    throw new Error(`unknown ctx.accounts quickfix was missing: ${JSON.stringify(unknownActions)}`);
  }
  const addUnknownAccountEdit = unknownActions
    ?.find((action) => action.title === "Add `acc` to `Create`")
    ?.edit?.changes?.[unknownCtxUri]?.[0]?.newText;
  if (addUnknownAccountEdit !== "    #[account(mut)]\n    pub acc: UncheckedAccount<'info>,\n") {
    throw new Error(`unknown ctx.accounts add-field quickfix was missing: ${JSON.stringify(unknownActions)}`);
  }

  openDocument(missingConstraintAccountUri, missingConstraintAccountSource);
  const missingConstraintAccountDiagnostics = await pullDiagnostics(missingConstraintAccountUri);
  const missingConstraintAccountDiagnostic = missingConstraintAccountDiagnostics?.items?.find(
    (item) =>
      item.code === "anchor-missing-account-reference" &&
      item.data?.constraint === "payer" &&
      item.data?.account === "user",
  );
  if (!missingConstraintAccountDiagnostic) {
    throw new Error(`missing constraint account reference was not diagnosed: ${JSON.stringify(missingConstraintAccountDiagnostics)}`);
  }
  if (
    !missingConstraintAccountDiagnostic.relatedInformation?.some((info) =>
      info.message?.includes("Accounts struct being checked"),
    )
  ) {
    throw new Error(`missing constraint account diagnostic did not include related Accounts evidence: ${JSON.stringify(missingConstraintAccountDiagnostic)}`);
  }
  const missingConstraintAccountActions = await request<CodeAction[]>("textDocument/codeAction", {
    textDocument: { uri: missingConstraintAccountUri },
    range: missingConstraintAccountDiagnostic.range,
    context: { diagnostics: [missingConstraintAccountDiagnostic], only: ["quickfix"] },
  });
  const addMissingConstraintAccountEdit = missingConstraintAccountActions
    ?.find((action) => action.title === "Add `user` to `Create`")
    ?.edit?.changes?.[missingConstraintAccountUri]?.[0]?.newText;
  if (addMissingConstraintAccountEdit !== "    #[account(mut)]\n    pub user: Signer<'info>,\n") {
    throw new Error(`missing constraint account add-field quickfix was missing: ${JSON.stringify(missingConstraintAccountActions)}`);
  }

  openDocument(missingAccountsUri, missingAccountsSource);
  const missingAccountsDiagnostics = await pullDiagnostics(missingAccountsUri);
  const missingAccountsDiagnostic = missingAccountsDiagnostics?.items?.find(
    (item) =>
      item.code === "anchor-context-accounts" &&
      item.data?.quickfix === "create-accounts-struct" &&
      item.data?.contextType === "MissingOffer",
  );
  if (!missingAccountsDiagnostic) {
    throw new Error(`missing accounts struct diagnostic did not include quickfix data: ${JSON.stringify(missingAccountsDiagnostics)}`);
  }
  if (
    !missingAccountsDiagnostic.relatedInformation?.some((info) =>
      info.message?.includes("uses `Context<MissingOffer>`"),
    )
  ) {
    throw new Error(`missing accounts struct diagnostic did not include handler related information: ${JSON.stringify(missingAccountsDiagnostic)}`);
  }
  const missingAccountsActions = await request<CodeAction[]>("textDocument/codeAction", {
    textDocument: { uri: missingAccountsUri },
    range: missingAccountsDiagnostic.range,
    context: { diagnostics: [missingAccountsDiagnostic], only: ["quickfix"] },
  });
  const createAccountsAction = missingAccountsActions?.find(
    (action) => action.title === "Create #[derive(Accounts)] struct MissingOffer",
  );
  const createAccountsEdit = createAccountsAction?.edit?.changes?.[missingAccountsUri]?.[0]?.newText;
  if (
    !createAccountsEdit?.includes("#[derive(Accounts)]") ||
    !createAccountsEdit.includes("pub struct MissingOffer<'info>") ||
    !createAccountsEdit.includes("#[account(mut)]") ||
    !createAccountsEdit.includes("pub maker: Signer<'info>,") ||
    !createAccountsEdit.includes("pub rent: Sysvar<'info, Rent>,") ||
    !createAccountsEdit.includes("pub system_program: Program<'info, System>,") ||
    !createAccountsEdit.includes("pub vault: UncheckedAccount<'info>,")
  ) {
    throw new Error(`create accounts struct quickfix was missing: ${JSON.stringify(missingAccountsActions)}`);
  }

  openDocument(emptyContextUri, emptyContextSource);
  const emptyContextDiagnostics = await pullDiagnostics(emptyContextUri);
  const emptyContextDiagnostic = emptyContextDiagnostics?.items?.find(
    (item) =>
      item.code === "anchor-context-accounts" &&
      item.data?.quickfix === "fill-context-type" &&
      item.data?.contextType === "FreshContextDemo",
  );
  if (!emptyContextDiagnostic) {
    throw new Error(`empty Context<> diagnostic did not include fill quickfix data: ${JSON.stringify(emptyContextDiagnostics)}`);
  }
  if (
    !emptyContextDiagnostic.relatedInformation?.some((info) =>
      info.message?.includes("empty Anchor `Context<>`"),
    )
  ) {
    throw new Error(`empty Context<> diagnostic did not include handler related information: ${JSON.stringify(emptyContextDiagnostic)}`);
  }
  const emptyContextActions = await request<CodeAction[]>("textDocument/codeAction", {
    textDocument: { uri: emptyContextUri },
    range: emptyContextDiagnostic.range,
    context: { diagnostics: [emptyContextDiagnostic], only: ["quickfix"] },
  });
  const combinedEmptyContextAction = emptyContextActions?.find(
    (action) => action.title === "Use `Context<FreshContextDemo>` and create Accounts struct",
  );
  const combinedEmptyContextEdits = combinedEmptyContextAction?.edit?.changes?.[emptyContextUri] ?? [];
  if (
    !combinedEmptyContextEdits.some((edit) => edit.newText === "FreshContextDemo") ||
    !combinedEmptyContextEdits.some((edit) => edit.newText.includes("pub struct FreshContextDemo<'info>"))
  ) {
    throw new Error(`combined empty Context<> quickfix was missing: ${JSON.stringify(emptyContextActions)}`);
  }

  openDocument(accountsAliasUri, accountsAliasSource);
  const accountsAliasDiagnostics = await pullDiagnostics(accountsAliasUri);
  const aliasMutDiagnostic = accountsAliasDiagnostics?.items?.find(
    (item) =>
      item.code === "anchor-account-usage" &&
      item.data?.quickfix === "add-mut-constraint" &&
      item.data?.account === "counter",
  );
  if (!aliasMutDiagnostic) {
    throw new Error(`accounts alias mutation diagnostic was missing: ${JSON.stringify(accountsAliasDiagnostics)}`);
  }
  const aliasPrepareRename = await request<PrepareRename>("textDocument/prepareRename", {
    textDocument: { uri: accountsAliasUri },
    position: positionAfter(accountsAliasSource, "accounts.counter"),
  });
  if (aliasPrepareRename?.placeholder !== "counter") {
    throw new Error(`prepareRename did not identify Anchor account field: ${JSON.stringify(aliasPrepareRename)}`);
  }
  const aliasRename = await request<WorkspaceEdit>("textDocument/rename", {
    textDocument: { uri: accountsAliasUri },
    position: positionAfter(accountsAliasSource, "accounts.counter"),
    newName: "asset",
  });
  const aliasRenameEdits = aliasRename?.changes?.[accountsAliasUri] ?? [];
  if (
    !aliasRenameEdits.some((edit) => edit.newText === "asset" && edit.range.start.line === 9) ||
    !aliasRenameEdits.some((edit) => edit.newText === "asset" && edit.range.start.line === 16)
  ) {
    throw new Error(`rename did not edit Anchor account field usage and declaration: ${JSON.stringify(aliasRename)}`);
  }

  openDocument(cpiUri, cpiSource);
  const cpiDiagnostics = await pullDiagnostics(cpiUri);
  const cpiDiagnostic = cpiDiagnostics.items?.find(
    (item) =>
      item.code === "anchor-security-cpi-program" &&
      item.data?.reason === "executable-cpi-program" &&
      item.data?.account === "metadata_program",
  );
  if (!cpiDiagnostic) {
    throw new Error(`CPI program usage diagnostic was missing: ${JSON.stringify(cpiDiagnostics)}`);
  }
  const cpiActions = await request<CodeAction[]>("textDocument/codeAction", {
    textDocument: { uri: cpiUri },
    range: cpiDiagnostic.range,
    context: { diagnostics: [cpiDiagnostic], only: ["quickfix"] },
  });
  const cpiExecutableAction = cpiActions?.find((action) => action.title?.includes("executable"));
  const cpiEdit = cpiExecutableAction?.edit?.changes?.[cpiUri]?.[0]?.newText?.trim();
  if (cpiEdit !== "#[account(executable)]") {
    throw new Error(`CPI program quickfix did not add executable constraint: ${JSON.stringify(cpiActions)}`);
  }

  openDocument(artifactSmokeUri, artifactSmokeSource);
  const artifactDiagnostics = await pullDiagnostics(artifactSmokeUri);
  assertNoDuplicateDiagnostics(artifactSmokeUri, artifactDiagnostics);
  for (const code of ["anchor-sbf-artifact", "anchor-program-keypair", "anchor-idl-artifact", "anchor-types-artifact"]) {
    if (!artifactDiagnostics.items?.some((item) => item.code === code)) {
      throw new Error(`artifact smoke diagnostics missing ${code}: ${JSON.stringify(artifactDiagnostics.items)}`);
    }
  }

  const artifactAnalysis = await request<AnalysisReport>("workspace/executeCommand", {
    command: "seagrass/analyze",
    arguments: [{ uri: artifactSmokeUri }],
  });
  if (
    artifactAnalysis?.project?.artifacts == null ||
    (artifactAnalysis.project.artifacts as JsonObject).deploy == null ||
    (artifactAnalysis.project.artifacts as JsonObject).idl == null
  ) {
    throw new Error(`analyze command did not return artifact evidence: ${JSON.stringify(artifactAnalysis)}`);
  }
  const artifactReport = await request<JsonObject>("workspace/executeCommand", {
    command: "seagrass/artifacts",
    arguments: [{ uri: artifactSmokeUri }],
  });
  if ((artifactReport.artifacts as JsonObject | undefined)?.deploy == null) {
    throw new Error(`artifacts command did not return per-document artifact evidence: ${JSON.stringify(artifactReport)}`);
  }
  notify("workspace/didChangeWatchedFiles", {
    changes: [
      {
        uri: pathToFileURL(resolve(artifactSmokeRoot, "target/deploy/artifact_demo.so")).href,
        type: 2,
      },
    ],
  });

  openDocument(pinocchioSmokeUri, pinocchioSmokeSource);
  const pinocchioDiagnostics = await pullDiagnostics(pinocchioSmokeUri);
  assertNoDuplicateDiagnostics(pinocchioSmokeUri, pinocchioDiagnostics);
  if (
    !pinocchioDiagnostics.items?.some(
      (item) =>
        item.code === "solana-code-quality" &&
        item.data?.rule === "solana/program-code-quality" &&
        item.data?.absorbedFrom === "solana-mcp-official/programAutofixer",
    )
  ) {
    throw new Error(`Pinocchio unchecked arithmetic diagnostic was missing: ${JSON.stringify(pinocchioDiagnostics.items)}`);
  }
  if (
    !pinocchioDiagnostics.items?.some(
      (item) =>
        item.code === "solana-code-quality" &&
        item.data?.attack === "instruction-data-bounds" &&
        item.data?.programKind === "pinocchio" &&
        item.data?.quickfix === "use-checked-data-access",
    )
  ) {
    throw new Error(`Pinocchio instruction-data bounds diagnostic was missing: ${JSON.stringify(pinocchioDiagnostics.items)}`);
  }
  if (
    !pinocchioDiagnostics.items?.some(
      (item) =>
        item.code === "anchor-sbf-artifact" &&
        item.data?.programKind === "pinocchio" &&
        item.data?.reason === "invalid" &&
        item.data?.buildCommand === "cargo build-sbf",
    )
  ) {
    throw new Error(`Pinocchio artifact diagnostic was missing: ${JSON.stringify(pinocchioDiagnostics.items)}`);
  }
  for (const code of ["anchor-idl-artifact", "anchor-types-artifact"]) {
    if (pinocchioDiagnostics.items?.some((item) => item.code === code)) {
      throw new Error(`Pinocchio project should not require Anchor IDL/types: ${JSON.stringify(pinocchioDiagnostics.items)}`);
    }
  }
  const pinocchioReport = await request<JsonObject>("workspace/executeCommand", {
    command: "seagrass/artifacts",
    arguments: [{ uri: pinocchioSmokeUri }],
  });
  const pinocchioArtifacts = pinocchioReport.artifacts as JsonObject | undefined;
  const pinocchioProgram = pinocchioArtifacts?.program as JsonObject | undefined;
  const pinocchioIdl = pinocchioArtifacts?.idl as JsonObject | undefined;
  const pinocchioDeploy = pinocchioArtifacts?.deploy as JsonObject | undefined;
  if (
    pinocchioReport.anchorToml !== null ||
    pinocchioProgram?.kind !== "pinocchio" ||
    pinocchioDeploy?.status !== "invalid" ||
    pinocchioIdl?.status !== "not-applicable"
  ) {
    throw new Error(`Pinocchio artifacts command returned wrong evidence: ${JSON.stringify(pinocchioReport)}`);
  }

  openDocument(nativeSmokeUri, nativeSmokeSource);
  const nativeDiagnostics = await pullDiagnostics(nativeSmokeUri);
  assertNoDuplicateDiagnostics(nativeSmokeUri, nativeDiagnostics);
  if (
    !nativeDiagnostics.items?.some(
      (item) =>
        item.code === "solana-code-quality" &&
        item.data?.attack === "bump-seed-canonicalization" &&
        item.data?.absorbedFrom === "coral-xyz/sealevel-attacks",
    )
  ) {
    throw new Error(`native canonical PDA bump diagnostic was missing: ${JSON.stringify(nativeDiagnostics.items)}`);
  }
  for (const attack of ["owner-checks", "type-cosplay", "instruction-data-bounds", "signer-authorization", "arbitrary-cpi"]) {
    if (
      !nativeDiagnostics.items?.some(
        (item) =>
          item.code === "solana-code-quality" &&
          item.data?.attack === attack,
      )
    ) {
      throw new Error(`native ${attack} diagnostic was missing: ${JSON.stringify(nativeDiagnostics.items)}`);
    }
  }
  const nativeBoundsDiagnostic = nativeDiagnostics.items?.find((item) => item.data?.attack === "instruction-data-bounds");
  if (!nativeBoundsDiagnostic) {
    throw new Error(`native bounds diagnostic was missing after attack loop: ${JSON.stringify(nativeDiagnostics.items)}`);
  }
  if (nativeBoundsDiagnostic?.data?.quickfix !== "use-checked-data-access") {
    throw new Error(`native bounds diagnostic did not include quickfix metadata: ${JSON.stringify(nativeBoundsDiagnostic)}`);
  }
  const nativeBoundsActions = await request<CodeAction[]>("textDocument/codeAction", {
    textDocument: { uri: nativeSmokeUri },
    range: nativeBoundsDiagnostic.range,
    context: { diagnostics: [nativeBoundsDiagnostic], only: ["quickfix"] },
  });
  if (!nativeBoundsActions?.some((action) => action.title?.includes("checked data access"))) {
    throw new Error(`native checked-data guidance quickfix was missing: ${JSON.stringify(nativeBoundsActions)}`);
  }
  if (
    !nativeDiagnostics.items?.some(
      (item) =>
        item.code === "anchor-sbf-artifact" &&
        item.data?.programKind === "native" &&
        item.data?.reason === "invalid" &&
        item.data?.buildCommand === "cargo build-sbf",
    )
  ) {
    throw new Error(`native Solana artifact diagnostic was missing: ${JSON.stringify(nativeDiagnostics.items)}`);
  }
  for (const code of ["anchor-idl-artifact", "anchor-types-artifact"]) {
    if (nativeDiagnostics.items?.some((item) => item.code === code)) {
      throw new Error(`native Solana project should not require Anchor IDL/types: ${JSON.stringify(nativeDiagnostics.items)}`);
    }
  }
  const nativeReport = await request<JsonObject>("workspace/executeCommand", {
    command: "seagrass/artifacts",
    arguments: [{ uri: nativeSmokeUri }],
  });
  const nativeArtifacts = nativeReport.artifacts as JsonObject | undefined;
  const nativeProgram = nativeArtifacts?.program as JsonObject | undefined;
  const nativePaths = nativeArtifacts?.paths as JsonObject | undefined;
  const nativeDeploy = nativeArtifacts?.deploy as JsonObject | undefined;
  const nativeDeployPath = nativePaths?.deploy;
  if (
    nativeReport.anchorToml !== null ||
    nativeProgram?.kind !== "native" ||
    nativeDeploy?.status !== "invalid" ||
    typeof nativeDeployPath !== "string" ||
    !nativeDeployPath.endsWith("target/deploy/native_counter_program.so")
  ) {
    throw new Error(`native Solana artifacts command returned wrong evidence: ${JSON.stringify(nativeReport)}`);
  }

  const cargoArtifactWorkspaceReport = await request<JsonObject>("workspace/executeCommand", {
    command: "seagrass/artifacts",
    arguments: [],
  });
  const cargoPrograms = cargoArtifactWorkspaceReport.programs;
  if (!Array.isArray(cargoPrograms)) {
    throw new Error(`workspace artifacts command did not return programs: ${JSON.stringify(cargoArtifactWorkspaceReport)}`);
  }
  if (
    !cargoPrograms.some(
      (program) => ((program as JsonObject).program as JsonObject | undefined)?.kind === "pinocchio",
    )
  ) {
    throw new Error(`workspace artifacts command missed Pinocchio program: ${JSON.stringify(cargoArtifactWorkspaceReport)}`);
  }
  if (
    !cargoPrograms.some(
      (program) => ((program as JsonObject).program as JsonObject | undefined)?.kind === "native",
    )
  ) {
    throw new Error(`workspace artifacts command missed native Solana program: ${JSON.stringify(cargoArtifactWorkspaceReport)}`);
  }

  openDocument(ecosystemSmokeUri, ecosystemSmokeSource);
  const ecosystemDiagnostics = await pullDiagnostics(ecosystemSmokeUri);
  assertNoDuplicateDiagnostics(ecosystemSmokeUri, ecosystemDiagnostics);
  for (const code of [
    "anchor-spl-token-interface",
    "solana-idl-artifact",
    "solana-program-metadata",
    "solana-test-harness",
    "solana-surfpool-workspace",
  ]) {
    if (!ecosystemDiagnostics.items?.some((item) => item.code === code)) {
      throw new Error(`ecosystem smoke diagnostics missing ${code}: ${JSON.stringify(ecosystemDiagnostics.items)}`);
    }
  }
  const ecosystemReport = await request<JsonObject>("workspace/executeCommand", {
    command: "seagrass/artifacts",
    arguments: [{ uri: ecosystemSmokeUri }],
  });
  const ecosystem = ecosystemReport.ecosystem as JsonObject | undefined;
  const idlSources = ecosystem?.idlSources;
  const testHarnesses = ecosystem?.testHarnesses;
  if (
    ecosystem == null ||
    !Array.isArray(idlSources) ||
    !idlSources.some((source) => (source as JsonObject).kind === "codama") ||
    !Array.isArray(testHarnesses) ||
    !testHarnesses.some((harness) => (harness as JsonObject).kind === "litesvm")
  ) {
    throw new Error(`ecosystem artifacts command returned wrong evidence: ${JSON.stringify(ecosystemReport)}`);
  }

  const status = await request<unknown>("workspace/executeCommand", {
    command: "seagrass/status",
    arguments: [],
  });
  if (typeof status !== "string" || !status.includes("indexed files")) {
    throw new Error(`status command did not return server status: ${JSON.stringify(status)}`);
  }

  const analysis = await request<AnalysisReport>("workspace/executeCommand", {
    command: "seagrass/analyze",
    arguments: [{ uri: mainUri }],
  });
  if (analysis?.uri !== mainUri || !analysis.evidence || !Array.isArray(analysis.diagnostics)) {
    throw new Error(`analyze command did not return document analysis: ${JSON.stringify(analysis)}`);
  }
  const focusedAnalysis = await request<AnalysisReport>("workspace/executeCommand", {
    command: "seagrass/analyze",
    arguments: [{ uri: mainUri, instruction: "initialize" }],
  });
  if (
    focusedAnalysis?.focus?.kind !== "anchor.programInstruction" ||
    focusedAnalysis.focus?.instruction !== "initialize" ||
    (focusedAnalysis.focus?.context as JsonObject | undefined)?.name !== "Create"
  ) {
    throw new Error(`analyze command did not return focused instruction analysis: ${JSON.stringify(focusedAnalysis)}`);
  }
  const instructionSummary = await request<JsonObject>("workspace/executeCommand", {
    command: "seagrass/instructionSummary",
    arguments: [{ uri: mainUri, function: "initialize" }],
  });
  const summaryAccounts = instructionSummary.accounts;
  if (
    instructionSummary?.name !== "initialize" ||
    instructionSummary?.context !== "Create" ||
    !Array.isArray(summaryAccounts) ||
    !summaryAccounts.some((account) => {
      const object = account as JsonObject;
      return object.name === "user" && object.ty === "Signer" && object.signer === true;
    }) ||
    !Array.isArray(instructionSummary.errorsReturned)
  ) {
    throw new Error(`seagrass instruction summary returned wrong shape: ${JSON.stringify(instructionSummary)}`);
  }
  const programReport = await request<JsonObject>("workspace/executeCommand", {
    command: "seagrass/programReport",
    arguments: [],
  });
  if (
    !Array.isArray(programReport?.programs) ||
    !Array.isArray(programReport?.instructions) ||
    !programReport.instructions.some((instruction) => {
      const object = instruction as JsonObject;
      return object.name === "initialize" && object.context === "Create";
    }) ||
    !Array.isArray(programReport?.pdas) ||
    !Array.isArray(programReport?.errors) ||
    !("idlHash" in programReport)
  ) {
    throw new Error(`seagrass program report returned wrong shape: ${JSON.stringify(programReport)}`);
  }
  openDocument(proactiveAssistUri, proactiveAssistSource);
  const assistRange = {
    start: { line: 0, character: 0 },
    end: { line: proactiveAssistSource.split("\n").length, character: 0 },
  };
  const proactiveActions = await request<CodeAction[]>("textDocument/codeAction", {
    textDocument: { uri: proactiveAssistUri },
    range: assistRange,
    context: { diagnostics: [], only: ["refactor"] },
  });
  const systemProgramAction = proactiveActions.find(
    (action) =>
      action.title === "Add Anchor system program account" &&
      action.kind === "refactor" &&
      action.isPreferred === true &&
      action.diagnostics == null &&
      action.data?.seagrassAssist === "add-system-program-field",
  );
  const expectedSystemProgramEdit = "    pub system_program: Program<'info, System>,\n";
  const systemProgramActionEdit = systemProgramAction?.edit?.changes?.[proactiveAssistUri]?.find(
    (edit) => edit.newText === expectedSystemProgramEdit,
  );
  if (
    systemProgramAction?.data?.evidence?.accountsStruct !== "Create" ||
    systemProgramAction?.data?.evidence?.field !== "system_program" ||
    systemProgramAction?.data?.evidence?.reason !==
      "init-like account constraints require the System program account" ||
    !systemProgramActionEdit
  ) {
    throw new Error(`proactive system program code action was missing: ${JSON.stringify(proactiveActions)}`);
  }
  const proposedAssists = await request<ProposedAssistsResponse>("workspace/executeCommand", {
    command: "seagrass/proposeAssists",
    arguments: [{ uri: proactiveAssistUri }],
  });
  const systemProgramAssist = proposedAssists.assists?.find(
    (assist) =>
      assist.id === "add-system-program-field" &&
      assist.kind === "refactor" &&
      assist.applicability === "machineApplicable" &&
      assist.hasEdit === true &&
      assist.evidence?.field === "system_program",
  );
  if (
    proposedAssists.uri !== proactiveAssistUri ||
    systemProgramAssist?.title !== "Add Anchor system program account" ||
    systemProgramAssist?.evidence?.accountsStruct !== "Create" ||
    systemProgramAssist?.evidence?.reason !== "init-like account constraints require the System program account" ||
    !systemProgramAssist?.edit?.changes?.[proactiveAssistUri]?.some(
      (edit) => edit.newText === expectedSystemProgramEdit,
    )
  ) {
    throw new Error(`seagrass propose assists returned wrong shape: ${JSON.stringify(proposedAssists)}`);
  }
  const expectedTokenProgramEdit = "    pub token_program: Program<'info, Token>,\n";
  const tokenProgramAction = proactiveActions.find(
    (action) =>
      action.title === "Add Anchor token program account" &&
      action.kind === "refactor" &&
      action.isPreferred === true &&
      action.diagnostics == null &&
      action.data?.seagrassAssist === "add-token-program-field",
  );
  if (
    tokenProgramAction?.data?.evidence?.accountsStruct !== "CreateAssociatedToken" ||
    tokenProgramAction?.data?.evidence?.field !== "token_program" ||
    tokenProgramAction?.data?.evidence?.reason !==
      "token or mint init constraints require the Token program account" ||
    !tokenProgramAction?.edit?.changes?.[proactiveAssistUri]?.some(
      (edit) => edit.newText === expectedTokenProgramEdit,
    )
  ) {
    throw new Error(`proactive token program code action was missing: ${JSON.stringify(proactiveActions)}`);
  }
  const tokenProgramAssist = proposedAssists.assists?.find(
    (assist) =>
      assist.id === "add-token-program-field" &&
      assist.title === "Add Anchor token program account" &&
      assist.kind === "refactor" &&
      assist.applicability === "machineApplicable" &&
      assist.hasEdit === true &&
      assist.evidence?.accountsStruct === "CreateAssociatedToken" &&
      assist.evidence?.field === "token_program" &&
      assist.evidence?.reason === "token or mint init constraints require the Token program account",
  );
  if (
    !tokenProgramAssist?.edit?.changes?.[proactiveAssistUri]?.some(
      (edit) => edit.newText === expectedTokenProgramEdit,
    )
  ) {
    throw new Error(`seagrass propose assists missed token program: ${JSON.stringify(proposedAssists)}`);
  }
  const expectedAssociatedTokenProgramEdit =
    "    pub associated_token_program: Program<'info, AssociatedToken>,\n";
  const associatedTokenProgramAction = proactiveActions.find(
    (action) =>
      action.title === "Add Anchor associated token program account" &&
      action.kind === "refactor" &&
      action.isPreferred === true &&
      action.diagnostics == null &&
      action.data?.seagrassAssist === "add-associated-token-program-field",
  );
  if (
    associatedTokenProgramAction?.data?.evidence?.accountsStruct !== "CreateAssociatedToken" ||
    associatedTokenProgramAction?.data?.evidence?.field !== "associated_token_program" ||
    associatedTokenProgramAction?.data?.evidence?.reason !==
      "associated token constraints require the Associated Token program account" ||
    !associatedTokenProgramAction?.edit?.changes?.[proactiveAssistUri]?.some(
      (edit) => edit.newText === expectedAssociatedTokenProgramEdit,
    )
  ) {
    throw new Error(`proactive associated token program code action was missing: ${JSON.stringify(proactiveActions)}`);
  }
  const associatedTokenProgramAssist = proposedAssists.assists?.find(
    (assist) =>
      assist.id === "add-associated-token-program-field" &&
      assist.title === "Add Anchor associated token program account" &&
      assist.kind === "refactor" &&
      assist.applicability === "machineApplicable" &&
      assist.hasEdit === true &&
      assist.evidence?.accountsStruct === "CreateAssociatedToken" &&
      assist.evidence?.field === "associated_token_program" &&
      assist.evidence?.reason === "associated token constraints require the Associated Token program account",
  );
  if (
    !associatedTokenProgramAssist?.edit?.changes?.[proactiveAssistUri]?.some(
      (edit) => edit.newText === expectedAssociatedTokenProgramEdit,
    )
  ) {
    throw new Error(`seagrass propose assists missed associated token program: ${JSON.stringify(proposedAssists)}`);
  }
  const pdaBumpAction = proactiveActions.find(
    (action) =>
      action.title === "Add Anchor PDA bump constraint" &&
      action.kind === "refactor" &&
      action.isPreferred === true &&
      action.diagnostics == null &&
      action.data?.seagrassAssist === "add-pda-bump-constraint" &&
      action.data?.evidence?.accountsStruct === "UpdateVault" &&
      action.data?.evidence?.field === "vault",
  );
  if (
    pdaBumpAction?.data?.evidence?.accountsStruct !== "UpdateVault" ||
    pdaBumpAction?.data?.evidence?.field !== "vault" ||
    pdaBumpAction?.data?.evidence?.constraint !== "bump" ||
    pdaBumpAction?.data?.evidence?.reason !== "PDA seeds should validate the canonical bump" ||
    !pdaBumpAction?.edit?.changes?.[proactiveAssistUri]?.some((edit) => edit.newText === ", bump")
  ) {
    throw new Error(`proactive PDA bump code action was missing: ${JSON.stringify(proactiveActions)}`);
  }
  const pdaBumpAssist = proposedAssists.assists?.find(
    (assist) =>
      assist.id === "add-pda-bump-constraint" &&
      assist.title === "Add Anchor PDA bump constraint" &&
      assist.kind === "refactor" &&
      assist.applicability === "machineApplicable" &&
      assist.hasEdit === true &&
      assist.evidence?.accountsStruct === "UpdateVault" &&
      assist.evidence?.field === "vault" &&
      assist.evidence?.constraint === "bump",
  );
  if (!pdaBumpAssist?.edit?.changes?.[proactiveAssistUri]?.some((edit) => edit.newText === ", bump")) {
    throw new Error(`seagrass propose assists missed PDA bump: ${JSON.stringify(proposedAssists)}`);
  }
  const mutConstraintAction = proactiveActions.find(
    (action) =>
      action.title === "Add Anchor mut constraint" &&
      action.kind === "refactor" &&
      action.isPreferred === true &&
      action.diagnostics == null &&
      action.data?.seagrassAssist === "add-mut-constraint",
  );
  if (
    mutConstraintAction?.data?.evidence?.accountsStruct !== "UpdateVault" ||
    mutConstraintAction?.data?.evidence?.field !== "vault" ||
    mutConstraintAction?.data?.evidence?.constraint !== "mut" ||
    mutConstraintAction?.data?.evidence?.instruction !== "update" ||
    !mutConstraintAction?.edit?.changes?.[proactiveAssistUri]?.some((edit) => edit.newText === ", mut")
  ) {
    throw new Error(`proactive mut constraint code action was missing: ${JSON.stringify(proactiveActions)}`);
  }
  const mutConstraintAssist = proposedAssists.assists?.find(
    (assist) =>
      assist.id === "add-mut-constraint" &&
      assist.title === "Add Anchor mut constraint" &&
      assist.kind === "refactor" &&
      assist.applicability === "machineApplicable" &&
      assist.hasEdit === true &&
      assist.evidence?.accountsStruct === "UpdateVault" &&
      assist.evidence?.field === "vault" &&
      assist.evidence?.constraint === "mut" &&
      assist.evidence?.instruction === "update",
  );
  if (!mutConstraintAssist?.edit?.changes?.[proactiveAssistUri]?.some((edit) => edit.newText === ", mut")) {
    throw new Error(`seagrass propose assists missed mut constraint: ${JSON.stringify(proposedAssists)}`);
  }
  const instructionArgsAction = proactiveActions.find(
    (action) =>
      action.title === "Add Anchor instruction arguments attribute" &&
      action.kind === "refactor" &&
      action.isPreferred === true &&
      action.diagnostics == null &&
      action.data?.seagrassAssist === "add-instruction-args-attribute",
  );
  const instructionArgs = instructionArgsAction?.data?.evidence?.arguments;
  if (
    instructionArgsAction?.data?.evidence?.accountsStruct !== "UpdateVault" ||
    !Array.isArray(instructionArgs) ||
    !(instructionArgs as JsonObject[]).some((argument) => argument.name === "name" && argument.type === "String") ||
    !instructionArgsAction?.edit?.changes?.[proactiveAssistUri]?.some(
      (edit) => edit.newText === "#[instruction(name: String)]\n",
    )
  ) {
    throw new Error(`proactive instruction args code action was missing: ${JSON.stringify(proactiveActions)}`);
  }
  const instructionArgsAssist = proposedAssists.assists?.find(
    (assist) =>
      assist.id === "add-instruction-args-attribute" &&
      assist.title === "Add Anchor instruction arguments attribute" &&
      assist.kind === "refactor" &&
      assist.applicability === "machineApplicable" &&
      assist.hasEdit === true &&
      assist.evidence?.accountsStruct === "UpdateVault",
  );
  const proposedInstructionArgs = instructionArgsAssist?.evidence?.arguments;
  if (
    !Array.isArray(proposedInstructionArgs) ||
    !(proposedInstructionArgs as JsonObject[]).some((argument) => argument.name === "name" && argument.type === "String") ||
    !instructionArgsAssist?.edit?.changes?.[proactiveAssistUri]?.some(
      (edit) => edit.newText === "#[instruction(name: String)]\n",
    )
  ) {
    throw new Error(`seagrass propose assists missed instruction args: ${JSON.stringify(proposedAssists)}`);
  }
  const canonicalSeedsAction = proactiveActions.find(
    (action) =>
      action.title === "Add canonical PDA seeds helper" &&
      action.kind === "refactor" &&
      action.isPreferred === true &&
      action.diagnostics == null &&
      action.data?.seagrassAssist === "add-canonical-seeds-struct" &&
      action.data?.evidence?.accountsStruct === "SettlePosition" &&
      action.data?.evidence?.field === "position",
  );
  if (
    canonicalSeedsAction?.data?.evidence?.accountsStruct !== "SettlePosition" ||
    canonicalSeedsAction?.data?.evidence?.field !== "position" ||
    canonicalSeedsAction?.data?.evidence?.helper !== "PositionSeeds" ||
    !canonicalSeedsAction?.edit?.changes?.[proactiveAssistUri]?.some(
      (edit) =>
        edit.newText.includes("pub struct PositionSeeds<'a>") &&
        edit.newText.includes("pub market: &'a Pubkey,") &&
        edit.newText.includes("pub authority: &'a Pubkey,") &&
        edit.newText.includes("pub position_name: &'a str,"),
    )
  ) {
    throw new Error(`proactive canonical seeds code action was missing: ${JSON.stringify(proactiveActions)}`);
  }
  const canonicalSeedsAssist = proposedAssists.assists?.find(
    (assist) =>
      assist.id === "add-canonical-seeds-struct" &&
      assist.title === "Add canonical PDA seeds helper" &&
      assist.kind === "refactor" &&
      assist.applicability === "machineApplicable" &&
      assist.hasEdit === true &&
      assist.evidence?.accountsStruct === "SettlePosition" &&
      assist.evidence?.field === "position" &&
      assist.evidence?.helper === "PositionSeeds",
  );
  if (
    !canonicalSeedsAssist?.edit?.changes?.[proactiveAssistUri]?.some((edit) =>
      edit.newText.includes("pub struct PositionSeeds<'a>"),
    )
  ) {
    throw new Error(`seagrass propose assists missed canonical seeds: ${JSON.stringify(proposedAssists)}`);
  }
  const typedCpiAction = proactiveActions.find(
    (action) =>
      action.title === "Use typed CPI program account" &&
      action.kind === "refactor" &&
      action.isPreferred === true &&
      action.diagnostics == null &&
      action.data?.seagrassAssist === "use-typed-cpi-program-account",
  );
  if (
    typedCpiAction?.data?.evidence?.accountsStruct !== "SettlePosition" ||
    typedCpiAction?.data?.evidence?.field !== "token_program" ||
    typedCpiAction?.data?.evidence?.expectedType !== "Program<'info, Token>" ||
    !typedCpiAction?.edit?.changes?.[proactiveAssistUri]?.some(
      (edit) => edit.newText === "Program<'info, Token>",
    )
  ) {
    throw new Error(`proactive typed CPI code action was missing: ${JSON.stringify(proactiveActions)}`);
  }
  const executableCpiAction = proactiveActions.find(
    (action) =>
      action.title === "Add executable CPI program constraint" &&
      action.kind === "refactor" &&
      action.isPreferred === true &&
      action.diagnostics == null &&
      action.data?.seagrassAssist === "add-cpi-program-executable-constraint",
  );
  if (
    executableCpiAction?.data?.evidence?.accountsStruct !== "SettlePosition" ||
    executableCpiAction?.data?.evidence?.field !== "metadata_program" ||
    executableCpiAction?.data?.evidence?.constraint !== "executable" ||
    !executableCpiAction?.edit?.changes?.[proactiveAssistUri]?.some(
      (edit) => edit.newText.trim() === "#[account(executable)]",
    )
  ) {
    throw new Error(`proactive executable CPI code action was missing: ${JSON.stringify(proactiveActions)}`);
  }
  for (const assistId of ["use-typed-cpi-program-account", "add-cpi-program-executable-constraint"]) {
    if (!proposedAssists.assists?.some((assist) => assist.id === assistId && assist.hasEdit === true)) {
      throw new Error(`seagrass propose assists missed ${assistId}: ${JSON.stringify(proposedAssists)}`);
    }
  }
  const positionCursor = positionAfter(proactiveAssistSource, "pub position");
  const focusedProactiveActions = await request<CodeAction[]>("textDocument/codeAction", {
    textDocument: { uri: proactiveAssistUri },
    range: { start: positionCursor, end: positionCursor },
    context: { diagnostics: [], only: ["refactor"] },
  });
  if (
    !focusedProactiveActions.some((action) => action.data?.seagrassAssist === "add-canonical-seeds-struct") ||
    focusedProactiveActions.some((action) => action.data?.seagrassAssist === "add-system-program-field")
  ) {
    throw new Error(`focused proactive code actions ignored cursor range: ${JSON.stringify(focusedProactiveActions)}`);
  }
  openDocument(proactiveRefreshUri, proactiveRefreshSource);
  const refreshRange = {
    start: { line: 0, character: 0 },
    end: { line: proactiveRefreshSource.split("\n").length, character: 0 },
  };
  const refreshBeforeActions = await request<CodeAction[]>("textDocument/codeAction", {
    textDocument: { uri: proactiveRefreshUri },
    range: refreshRange,
    context: { diagnostics: [], only: ["refactor"] },
  });
  if (!refreshBeforeActions.some((action) => action.data?.seagrassAssist === "add-system-program-field")) {
    throw new Error(`proactive refresh fixture did not start with system assist: ${JSON.stringify(refreshBeforeActions)}`);
  }
  notify("textDocument/didChange", {
    textDocument: { uri: proactiveRefreshUri, version: 2 },
    contentChanges: [{ text: proactiveRefreshUpdatedSource }],
  });
  await pullDiagnostics(proactiveRefreshUri);
  const refreshAfterRange = {
    start: { line: 0, character: 0 },
    end: { line: proactiveRefreshUpdatedSource.split("\n").length, character: 0 },
  };
  const refreshAfterActions = await request<CodeAction[]>("textDocument/codeAction", {
    textDocument: { uri: proactiveRefreshUri },
    range: refreshAfterRange,
    context: { diagnostics: [], only: ["refactor"] },
  });
  if (refreshAfterActions.some((action) => action.data?.seagrassAssist === "add-system-program-field")) {
    throw new Error(`proactive code actions did not refresh after document edit: ${JSON.stringify(refreshAfterActions)}`);
  }
  const focusedContextAnalysis = await request<AnalysisReport>("workspace/executeCommand", {
    command: "seagrass/analyze",
    arguments: [{ uri: mainUri, context: "Create" }],
  });
  if (
    focusedContextAnalysis?.focus?.kind !== "anchor.accountsContext" ||
    focusedContextAnalysis.focus?.context !== "Create" ||
    !Array.isArray(focusedContextAnalysis.focus?.fields) ||
    !Array.isArray(focusedContextAnalysis.focus?.instructions)
  ) {
    throw new Error(`analyze command did not return focused accounts context analysis: ${JSON.stringify(focusedContextAnalysis)}`);
  }
  const errorCoverage = await request<JsonObject>("workspace/executeCommand", {
    command: "seagrass/errorCoverage",
    arguments: [],
  });
  if (!Array.isArray(errorCoverage?.errors) || errorCoverage.errors.length === 0) {
    throw new Error(`error coverage command did not return generated Anchor errors: ${JSON.stringify(errorCoverage)}`);
  }
  const supportMatrix = await request<JsonObject>("workspace/executeCommand", {
    command: "seagrass/supportMatrix",
    arguments: [],
  });
  if (!Array.isArray(supportMatrix?.constraints) || supportMatrix.constraints.length === 0) {
    throw new Error(`support matrix command did not return Anchor constraint coverage: ${JSON.stringify(supportMatrix)}`);
  }
  if (
    !Array.isArray(supportMatrix?.capabilities) ||
    !supportMatrix.capabilities.some((entry) => {
      const capability = entry as JsonObject;
      return capability.key === "cpiProgramSecurity" &&
        Array.isArray(capability.semanticChecks) &&
        capability.semanticChecks.includes("reachableSplitHelperUsage");
    }) ||
    !Array.isArray(supportMatrix?.capabilityGaps)
  ) {
    throw new Error(`support matrix command did not return capability coverage: ${JSON.stringify(supportMatrix)}`);
  }
  if (
    !Array.isArray(supportMatrix?.securityPatternCoverage) ||
    !["owner-checks", "type-cosplay", "instruction-data-bounds", "pda-seed-collision"].every((key) =>
      supportMatrix.securityPatternCoverage.some((entry) => {
        const coverage = entry as JsonObject;
        return coverage.key === key && coverage.status === "covered";
      }),
    )
  ) {
    throw new Error(`support matrix command did not return security pattern coverage: ${JSON.stringify(supportMatrix)}`);
  }
  if (
    !Array.isArray(supportMatrix?.securityRegressionCorpus) ||
    !supportMatrix.securityRegressionCorpus.some((entry) => {
      const regression = entry as JsonObject;
      return regression.name === "alias-raw-account-owner";
    })
  ) {
    throw new Error(`support matrix command did not return security regression corpus: ${JSON.stringify(supportMatrix)}`);
  }
  const projectCoverage = await request<JsonObject>("workspace/executeCommand", {
    command: "seagrass/projectCoverage",
    arguments: [],
  });
  if (
    !projectCoverage?.settings ||
    !Array.isArray(projectCoverage?.securityPatternCoverage) ||
    typeof projectCoverage?.diagnosticsByCode !== "object" ||
    typeof projectCoverage?.diagnosticsByAttack !== "object"
  ) {
    throw new Error(`project coverage command did not return editor coverage evidence: ${JSON.stringify(projectCoverage)}`);
  }
  const generatorProfile = await request<JsonObject>("workspace/executeCommand", {
    command: "seagrass/generatorProfile",
    arguments: [],
  });
  if (
    generatorProfile?.generationMode !== "source-driven" ||
    typeof generatorProfile?.coverageInputs !== "object" ||
    typeof generatorProfile?.fingerprints !== "object"
  ) {
    throw new Error(`generator profile command did not return source metadata: ${JSON.stringify(generatorProfile)}`);
  }
  const logs = await request<{ entries?: JsonObject[] }>("workspace/executeCommand", {
    command: "seagrass/logs",
    arguments: [],
  });
  if (!Array.isArray(logs.entries) || !logs.entries.some((entry) => entry.event === "initialized")) {
    throw new Error(`logs command did not include initialized event: ${JSON.stringify(logs)}`);
  }
  if (
    !logs.entries.some(
      (entry) =>
        entry.event === "navigation" &&
        entry.data?.method === "textDocument/implementation" &&
        entry.data?.resultCount === 1,
    )
  ) {
    throw new Error(`logs command did not include successful implementation navigation metadata: ${JSON.stringify(logs)}`);
  }
  if (
    !logs.entries.some(
      (entry) =>
        entry.event === "completionServed" &&
        entry.data?.triggerCharacter === "." &&
        entry.data?.gate === "accepted" &&
        typeof entry.data?.resultCount === "number" &&
        entry.data.resultCount > 0 &&
        (entry.data?.signature as JsonObject | undefined)?.kind === "AccountPath",
    )
  ) {
    throw new Error(`logs command did not include accepted dot-trigger account-path completion telemetry: ${JSON.stringify(logs)}`);
  }

  await request<null>("shutdown");
  if (process.env.SEAGRASS_WAIT_FOR_EXIT === "1") {
    server.stdin.end();
    await waitForServerExit();
  } else {
    notify("exit", {});
  }
  console.log("seagrass protocol smoke passed");
} finally {
  if (!serverExited) {
    server.kill();
  }
}
