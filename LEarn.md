Microchains
This section provides an introduction to microchains, the main building block of the Linera Protocol. For a more formal treatment refer to the whitepaper.

Background
A microchain is a chain of blocks describing successive changes to a shared state. We will use the terms chain and microchain interchangeably. Linera microchains are similar to the familiar notion of blockchain, with the following important specificities:

An arbitrary number of microchains can coexist in a Linera network, all sharing the same set of validators and the same level of security. Creating a new microchain only takes one transaction on an existing chain.

The task of proposing new blocks in a microchain can be assumed either by validators or by end users (or rather their wallets) depending on the configuration of a chain. Specifically, microchains can be single-owner, multi-owner, or public, depending on who is authorized to propose blocks.

Cross-chain messaging
In traditional networks with a single blockchain, every transaction can access the application's entire execution state. This is not the case in Linera where the state of an application is spread across multiple microchains, and the state on any individual microchain is only affected by the blocks of that microchain.

Cross-chain messaging is a way for different microchains to communicate with each other asynchronously. This method allows applications and data to be distributed across multiple chains for better scalability. When an application on one chain sends a message to itself on another chain, a cross-chain request is created. These requests are implemented using remote procedure calls (RPCs) within the validators' internal network, ensuring that each request is executed only once.

Instead of immediately modifying the target chain, messages are placed first in the target chain's inbox. When an owner of the target chain creates its next block in the future, they may reference a selection of messages taken from the current inbox in the new block. This executes the selected messages and applies their messages to the chain state.

Below is an example set of chains sending asynchronous messages to each other over consecutive blocks.

                               ┌───┐     ┌───┐     ┌───┐
                       Chain A │   ├────►│   ├────►│   │
                               └───┘     └───┘     └───┘
                                                     ▲
                                           ┌─────────┘
                                           │
                               ┌───┐     ┌─┴─┐     ┌───┐
                       Chain B │   ├────►│   ├────►│   │
                               └───┘     └─┬─┘     └───┘
                                           │         ▲
                                           │         │
                                           ▼         │
                               ┌───┐     ┌───┐     ┌─┴─┐
                       Chain C │   ├────►│   ├────►│   │
                               └───┘     └───┘     └───┘
The Linera protocol allows receivers to discard messages but not to change the ordering of selected messages inside the communication queue between two chains. If a selected message fails to execute, the wallet will automatically reject it when proposing the receiver's block. The current implementation of the Linera client always selects as many messages as possible from inboxes, and never discards messages unless they fail to execute.

Chain ownership semantics
Active chains can have one or multiple owners. Chains with zero owners are permanently deactivated.

In Linera, the validators guarantee safety: On each chain, at each height, there is at most one unique block.

But liveness—actually adding blocks to a chain at all—relies on the owners. There are different types of rounds and owners, optimized for different use cases:

First an optional fast round, where a super owner can propose blocks that get confirmed with very particularly low latency, optimal for single-owner chains with no contention.
Then a number of multi-leader rounds, where all regular owners can propose blocks. This works well even if there is occasional, temporary contention: an owner using multiple devices, or multiple people using the same chain infrequently.
And finally single-leader rounds: These give each regular chain owner a time slot in which only they can propose a new block, without being hindered by any other owners' proposals. This is ideal for chains with many users that are trying to commit blocks at the same time.
The number of multi-leader rounds is configurable: On chains with fluctuating levels of activity, this allows the system to dynamically switch to single-leader mode whenever all multi-leader rounds fail during periods of high contention. Chains that very often have high activity from multiple owners can set the number of multi-leader rounds to 0.

For more detail and examples on how to open and close chains, see the wallet section on chain management.

Common Design Patterns
We now explore some common design patterns to take advantage of microchains.

Applications with only user chains
Some applications such as payments only require user chains, hence are fully horizontally scalable:

Validators

initiates transfer

sends assets

notifies

user 1

user chain 1

user chain 2

user 2

Example: the fungible demo application of the Linera codebase.

Client/server applications
Pre-existing applications (e.g. written in Solidity) generally run on a single chain of blocks for all users. Those can be embedded in an app chain to act as a service.

Validators

initiates request

initiate response(s)

notifies

notifies

sends request

sends response

user 1

user chain 1

block producer

app chain

user chain 2

Depending on the nature of the application, the blocks produced in the app chain may be restricted to only contain messages (no operations). This is to ensure that block producers have no influence on a chain, other than selecting incoming messages.

Example: the crowd-funding demo application of the Linera codebase.

Using personal chains to scale applications
User chains are useful to store the assets of their users and initiate requests to app chains. Yet, oftentimes, they can also help applications scale horizontally by taking work out of the app chains.

Microchains

submits ZK proof

sends trusted message《ZK proof is valid》

sends tokens

user 1

user chain 1

airdrop chain

user chain 2

One of the benefits of personal chains is to enable transactions that would be too slow or not deterministic enough for traditional blockchains, including:

Validating ZK proofs,
Sending web queries to external oracle services (e.g. AI inference) and other API providers,
Downloading data blobs from external data availability (”DA”) layers and computing app-specific invariants.
Example (unfinished): the airdrop demo application of the Linera project.

Using temporary chains to scale applications
Temporary chains can be created on demand and configured to accept blocks from specific users.

The following diagram allows a virtually unlimited number of games (e.g. chess game) to be spawned for a given tournament.

Microchains

creates

reports result

request game

request game

plays

plays

user chain 1

tournament app chain

user chain 2

temporary game chain

user 1

user 2

Example: the hex-game demo application of the Linera codebase.

Just-in-time oracles
We have seen that Linera clients are connected and don’t rely on external RPC providers to read on-chain data from the chain. This ability to receive secure, censorship-resistant notifications and read data from the network is a game changer allowing on-chain applications to query certain clients in real time.

For instance, clients may be running an AI oracle off-chain in a trusted execution environment (TEE), allowing on-chain application to extract important information form the Internet.



Creating a Linera Project
To create your Linera project, use the linera project new command. The command should be executed outside the linera-protocol folder. It sets up the scaffolding and requisite files:

linera project new my-counter
linera project new bootstraps your project by creating the following key files:

Cargo.toml: your project's manifest filled with the necessary dependencies to create an app;
rust-toolchain.toml: a file with configuration for Rust compiler.
NOTE: currently the latest Rust version compatible with our network is 1.86.0. Make sure it's the one used by your project.

src/lib.rs: the application's ABI definition;
src/state.rs: the application's state;
src/contract.rs: the application's contract, and the binary target for the contract bytecode;
src/service.rs: the application's service, and the binary target for the service bytecode.
When writing Linera applications it is a convention to use your app's name as a prefix for names of trait, struct, etc. Hence, in the following manual, we will use CounterContract, CounterService, etc.




Creating the Application State
The state of a Linera application consists of onchain data that are persisted between transactions.

The struct which defines your application's state can be found in src/state.rs. To represent our counter, we're going to use a u64 integer.

While we could use a plain data-structure for the entire application state:

struct Counter {
  value: u64
}
in general, we prefer to manage persistent data using the concept of "views":

Views allow an application to load persistent data in memory and stage modifications in a flexible way.

Views resemble the persistent objects of an ORM framework, except that they are stored as a set of key-value pairs (instead of a SQL row).

In this case, the struct in src/state.rs should be replaced by

/// The application state.
#[derive(RootView, async_graphql::SimpleObject)]
#[view(context = ViewStorageContext)]
pub struct Counter {
    pub value: RegisterView<u64>,
    // Additional fields here will get their own key in storage.
}
and the occurrences of Application in the rest of the project should be replaced by Counter.

The derive macro async_graphql::SimpleObject is related to GraphQL queries discussed in the next section.

A RegisterView<T> supports modifying a single value of type T. Other data structures available in the library linera_views include:

LogView for a growing vector of values;
QueueView for queues;
MapView and CollectionView for associative maps; specifically, MapView in the case of static values, and CollectionView when values are other views.
For an exhaustive list of the different constructions, refer to the crate documentation




Defining the ABI
The Application Binary Interface (ABI) of a Linera application defines how to interact with this application from other parts of the system. It includes the data structures, data types, and functions exposed by on-chain contracts and services.

ABIs are usually defined in src/lib.rs and compiled across all architectures (Wasm and native).

For a reference guide, check out the documentation of the crate.

Defining a marker struct
The library part of your application (generally in src/lib.rs) must define a public empty struct that implements the Abi trait.

struct CounterAbi;
The Abi trait combines the ContractAbi and ServiceAbi traits to include the types that your application exports.

/// A trait that includes all the types exported by a Linera application (both contract
/// and service).
pub trait Abi: ContractAbi + ServiceAbi {}
Next, we're going to implement each of the two traits.

Contract ABI
The ContractAbi trait defines the data types that your application uses in a contract. Each type represents a specific part of the contract's behavior:

/// A trait that includes all the types exported by a Linera application contract.
pub trait ContractAbi {
    /// The type of operation executed by the application.
    ///
    /// Operations are transactions directly added to a block by the creator (and signer)
    /// of the block. Users typically use operations to start interacting with an
    /// application on their own chain.
    type Operation: Serialize + DeserializeOwned + Send + Sync + Debug + 'static;

    /// The response type of an application call.
    type Response: Serialize + DeserializeOwned + Send + Sync + Debug + 'static;

    /// How the `Operation` is deserialized
    fn deserialize_operation(operation: Vec<u8>) -> Result<Self::Operation, String> {
        bcs::from_bytes(&operation)
            .map_err(|e| format!("BCS deserialization error {e:?} for operation {operation:?}"))
    }

    /// How the `Operation` is serialized
    fn serialize_operation(operation: &Self::Operation) -> Result<Vec<u8>, String> {
        bcs::to_bytes(operation)
            .map_err(|e| format!("BCS serialization error {e:?} for operation {operation:?}"))
    }

    /// How the `Response` is deserialized
    fn deserialize_response(response: Vec<u8>) -> Result<Self::Response, String> {
        bcs::from_bytes(&response)
            .map_err(|e| format!("BCS deserialization error {e:?} for response {response:?}"))
    }

    /// How the `Response` is serialized
    fn serialize_response(response: Self::Response) -> Result<Vec<u8>, String> {
        bcs::to_bytes(&response)
            .map_err(|e| format!("BCS serialization error {e:?} for response {response:?}"))
    }
}
All these types must implement the Serialize, DeserializeOwned, Send, Sync, Debug traits, and have a 'static lifetime.

In our example, we would like to change our Operation to u64, like so:

pub struct CounterAbi;

impl ContractAbi for CounterAbi {
    type Operation = u64;
    type Response = u64;
}
Service ABI
The ServiceAbi is in principle very similar to the ContractAbi, just for the service component of your application.

The ServiceAbi trait defines the types used by the service part of your application:

/// A trait that includes all the types exported by a Linera application service.
pub trait ServiceAbi {
    /// The type of a query receivable by the application's service.
    type Query: Serialize + DeserializeOwned + Send + Sync + Debug + 'static;

    /// The response type of the application's service.
    type QueryResponse: Serialize + DeserializeOwned + Send + Sync + Debug + 'static;
}
For our Counter example, we'll be using GraphQL to query our application so our ServiceAbi should reflect that:

use async_graphql::{Request, Response};

impl ServiceAbi for CounterAbi {
    type Query = Request;
    type QueryResponse = Response;
}
References
The full trait definition of Abi can be found here.

The full Counter example application can be found here.


Writing the Contract Binary
The contract binary is the first component of a Linera application. It can actually change the state of the application.

To create a contract, we need to create a new type and implement the Contract trait for it, which is as follows:

pub trait Contract: WithContractAbi + ContractAbi + Sized {
    /// The type of message executed by the application.
    type Message: Serialize + DeserializeOwned + Debug;

    /// Immutable parameters specific to this application (e.g. the name of a token).
    type Parameters: Serialize + DeserializeOwned + Clone + Debug;

    /// Instantiation argument passed to a new application on the chain that created it
    /// (e.g. an initial amount of tokens minted).
    type InstantiationArgument: Serialize + DeserializeOwned + Debug;

    /// Event values for streams created by this application.
    type EventValue: Serialize + DeserializeOwned + Debug;

    /// Creates an in-memory instance of the contract handler.
    async fn load(runtime: ContractRuntime<Self>) -> Self;

    /// Instantiates the application on the chain that created it.
    async fn instantiate(&mut self, argument: Self::InstantiationArgument);

    /// Applies an operation from the current block.
    async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response;

    /// Applies a message originating from a cross-chain message.
    async fn execute_message(&mut self, message: Self::Message);

    /// Reacts to new events on streams.
    ///
    /// This is called whenever there is a new event on any stream that this application
    /// subscribes to.
    async fn process_streams(&mut self, _updates: Vec<StreamUpdate>) {}

    /// Finishes the execution of the current transaction.
    async fn store(self);
}
There's quite a bit going on here, so let's break it down and take one method at a time.

For this application, we'll be using the load, execute_operation and store methods.

The contract lifecycle
To implement the application contract, we first create a type for the contract:

linera_sdk::contract!(CounterContract);

pub struct CounterContract {
    state: CounterState,
    runtime: ContractRuntime<Self>,
}
This type usually contains at least two fields: the persistent state defined earlier and a handle to the runtime. The runtime provides access to information about the current execution and also allows sending messages, among other things. Other fields can be added, and they can be used to store volatile data that only exists while the current transaction is being executed, and discarded afterwards.

When a transaction is executed, the contract type is created through a call to Contract::load method. This method receives a handle to the runtime that the contract can use, and should use it to load the application state. For our implementation, we will load the state and create the CounterContract instance:

    async fn load(runtime: ContractRuntime<Self>) -> Self {
        let state = CounterState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        CounterContract { state, runtime }
    }
When the transaction finishes executing successfully, there's a final step where all loaded application contracts are called in order to do any final checks and persist its state to storage. That final step is a call to the Contract::store method, which can be thought of as similar to executing a destructor. In our implementation we will persist the state back to storage:

    async fn store(mut self) {
        self.state.save().await.expect("Failed to save state");
    }
It's possible to do more than just saving the state, and the Contract finalization section provides more details on that.

Instantiating our Application
The first thing that happens when an application is created from a bytecode is that it is instantiated. This is done by calling the contract's Contract::instantiate method.

Contract::instantiate is only called once when the application is created and only on the microchain that created the application.

Deployment on other microchains will use the Default value of all sub-views in the state if the state uses the view paradigm.

For our example application, we'll want to initialize the state of the application to an arbitrary number that can be specified on application creation using its instantiation parameters:

    async fn instantiate(&mut self, value: u64) {
        // Validate that the application parameters were configured correctly.
        self.runtime.application_parameters();

        self.state.value.set(value);
    }
Implementing the increment operation
Now that we have our counter's state and a way to initialize it to any value we would like, we need a way to increment our counter's value. Execution requests from block proposers or other applications are broadly called 'operations'.

To handle an operation, we need to implement the Contract::execute_operation method. In the counter's case, the operation it will be receiving is a u64 which is used to increment the counter by that value:

    async fn execute_operation(&mut self, operation: u64) -> u64 {
        let new_value = self.state.value.get() + operation;
        self.state.value.set(new_value);
        new_value
    }
Declaring the ABI
Finally, we link our Contract trait implementation with the ABI of the application:

impl WithContractAbi for CounterContract {
    type Abi = CounterAbi;
}
References
The full trait definition of Contract can be found here.

The full Counter examp



Writing the Service Binary
The service binary is the second component of a Linera application. It is compiled into a separate Bytecode from the contract and is run independently. It is not metered (meaning that querying an application's service does not consume gas), and can be thought of as a read-only view into your application.

Application states can be arbitrarily complex, and most of the time you don't want to expose this state in its entirety to those who would like to interact with your app. Instead, you might prefer to define a distinct set of queries that can be made against your application.

The Service trait is how you define the interface into your application. The Service trait is defined as follows:

pub trait Service: WithServiceAbi + ServiceAbi + Sized {
    /// Immutable parameters specific to this application.
    type Parameters: Serialize + DeserializeOwned + Send + Sync + Clone + Debug + 'static;

    /// Creates an in-memory instance of the service handler.
    async fn new(runtime: ServiceRuntime<Self>) -> Self;

    /// Executes a read-only query on the state of this application.
    async fn handle_query(&self, query: Self::Query) -> Self::QueryResponse;
}
Let's implement Service for our counter application.

First, we create a new type for the service, similarly to the contract:

linera_sdk::service!(CounterService);

pub struct CounterService {
    state: CounterState,
    runtime: Arc<ServiceRuntime<Self>>,
}
Just like with the CounterContract type, this type usually has two types: the application state and the runtime. We can omit the fields if we don't use them, so in this example we're omitting the runtime field, since its only used when constructing the CounterService type.

As before, the macro service! generates the necessary boilerplate for implementing the service WIT interface, exporting the necessary resource types and functions so that the service can be executed.

Next, we need to implement the Service trait for CounterService type. The first step is to define the Service's associated type, which is the global parameters specified when the application is instantiated. In our case, the global parameters aren't used, so we can just specify the unit type:

impl Service for CounterService {
    type Parameters = ();

    // ...
}
Also like in contracts, we must implement a load constructor when implementing the Service trait. The constructor receives the runtime handle and should use it to load the application state:

    async fn new(runtime: ServiceRuntime<Self>) -> Self {
        let state = CounterState::load(runtime.root_view_storage_context())
            .await
            .expect("Failed to load state");
        CounterService {
            state,
            runtime: Arc::new(runtime),
        }
    }
Services don't have a store method because they are read-only and can't persist any changes back to the storage.

The actual functionality of the service starts in the handle_query method. We will accept GraphQL queries and handle them using the async-graphql crate. To forward the queries to custom GraphQL handlers we will implement in the next section, we use the following code:

    async fn handle_query(&self, request: Request) -> Response {
        let schema = Schema::build(
            QueryRoot {
                value: *self.state.value.get(),
            },
            MutationRoot {
                runtime: self.runtime.clone(),
            },
            EmptySubscription,
        )
        .finish();
        schema.execute(request).await
    }
Finally, as before, the following code is needed to incorporate the ABI definitions into your Service implementation:

impl WithServiceAbi for CounterService {
    type Abi = counter::CounterAbi;
}
Adding GraphQL compatibility
Finally, we want our application to have GraphQL compatibility. To achieve this we need a QueryRoot to respond to queries and a MutationRoot for creating serialized Operation values that can be placed in blocks.

In the QueryRoot, we only create a single value query that returns the counter's value:

struct QueryRoot {
    value: u64,
}

#[Object]
impl QueryRoot {
    async fn value(&self) -> &u64 {
        &self.value
    }
}
In the MutationRoot, we only create one increment method that returns a serialized operation to increment the counter by the provided value:

struct MutationRoot {
    runtime: Arc<ServiceRuntime<CounterService>>,
}

#[Object]
impl MutationRoot {
    async fn increment(&self, value: u64) -> [u8; 0] {
        self.runtime.schedule_operation(&value);
        []
    }
}
We haven't included the imports in the above code. If you want the full source code and associated tests check out the examples section on GitHub.

References
The full trait definition of Service can be found here.

The full Counter example application can be found here.



Cross-Chain Messages
On Linera, applications are meant to be multi-chain: They are instantiated on every chain where they are used. An application has the same application ID and bytecode everywhere, but a separate state on every chain. To coordinate, the instances can send cross-chain messages to each other. A message sent by an application is always handled by the same application on the target chain: The handling code is guaranteed to be the same as the sending code, but the state may be different.

For your application, you can specify any serializable type as the Message type in your Contract implementation. To send a message, use the ContractRuntime made available as an argument to the contract's Contract::load constructor. The runtime is usually stored inside the contract object, as we did when writing the contract binary. We can then call ContractRuntime::prepare_message to start preparing a message, and then send_to to send it to a destination chain.

    self.runtime
        .prepare_message(message_contents)
        .send_to(destination_chain_id);
After block execution in the sending chain, sent messages are placed in the target chains' inboxes for processing. There is no guarantee that it will be handled: For this to happen, an owner of the target chain needs to include it in the incoming_messages in one of their blocks. When that happens, the contract's execute_message method gets called on their chain.

While preparing the message to be sent, it is possible to enable authentication forwarding and/or tracking. Authentication forwarding means that the message is executed by the receiver with the same authenticated signer as the sender of the message, while tracking means that the message is sent back to the sender if the receiver rejects it. The example below enables both flags:

    self.runtime
        .prepare_message(message_contents)
        .with_tracking()
        .with_authentication()
        .send_to(destination_chain_id);
Example: fungible token
In the fungible example application, such a message can be the transfer of tokens from one chain to another. If the sender includes a Transfer operation on their chain, it decreases their account balance and sends a Credit message to the recipient's chain:

async fn execute_operation(&mut self, operation: Self::Operation) -> Self::Response {
        match operation {
            FungibleOperation::Transfer {
                owner,
                amount,
                target_account,
            } => {
                self.runtime
                    .check_account_permission(owner)
                    .expect("Permission for Transfer operation");
                self.state.debit(owner, amount).await;
                self.finish_transfer_to_account(amount, target_account, owner)
                    .await;
                FungibleResponse::Ok
            }
            // ...
        }
}
    /// Executes the final step of a transfer where the tokens are sent to the destination.
    async fn finish_transfer_to_account(
        &mut self,
        amount: Amount,
        target_account: Account,
        source: AccountOwner,
    ) {
        if target_account.chain_id == self.runtime.chain_id() {
            self.state.credit(target_account.owner, amount).await;
        } else {
            let message = Message::Credit {
                target: target_account.owner,
                amount,
                source,
            };
            self.runtime
                .prepare_message(message)
                .with_authentication()
                .with_tracking()
                .send_to(target_account.chain_id);
        }
    }
On the recipient's chain, execute_message is called, which increases their account balance.

    async fn execute_message(&mut self, message: Message) {
        match message {
            Message::Credit {
                amount,
                target,
                source,
            } => {
                let is_bouncing = self
                    .runtime
                    .message_is_bouncing()
                    .expect("Delivery status is available when executing a message");
                let receiver = if is_bouncing { source } else { target };
                self.state.credit(receiver, amount).await;
            }
            // ...
        }
    }



    Calling other Applications
We have seen that cross-chain messages sent by an application on one chain are always handled by the same application on the target chain.

This section is about calling other applications using cross-application calls.

Such calls happen on the same chain and are made with the helper method ContractRuntime::call_application:

    pub fn call_application<A: ContractAbi + Send>(
        &mut self,
        authenticated: bool,
        application: ApplicationId<A>,
        call: &A::Operation,
    ) -> A::Response
The authenticated argument specifies whether the callee is allowed to perform actions that require authentication either

on behalf of the signer of the original block that caused this call, or
on behalf of the calling application.
The application argument is the callee's application ID, and A is the callee's ABI.

The call argument is the operation requested by the application call.

Example: crowd-funding
The crowd-funding example application allows the application creator to launch a campaign with a funding target. That target can be an amount specified in any type of token based on the fungible application. Others can then pledge tokens of that type to the campaign, and if the target is not reached by the deadline, they are refunded.

If Alice used the fungible example to create a Pugecoin application (with an impressionable pug as its mascot), then Bob can create a crowd-funding application, use Pugecoin's application ID as CrowdFundingAbi::Parameters, and specify in CrowdFundingAbi::InstantiationArgument that his campaign will run for one week and has a target of 1000 Pugecoins.

Now let's say Carol wants to pledge 10 Pugecoin tokens to Bob's campaign. She can make her pledge by running the linera service and making a query to Bob's application:

mutation { pledge(owner: "User:841…6c0", amount: "10") }
This will add a block to Carol's chain containing the pledge operation that gets handled by CrowdFunding::execute_operation, resulting in one cross-application call and two cross-chain messages:

First CrowdFunding::execute_operation calls the fungible application on Carol's chain to transfer 10 tokens to Carol's account on Bob's chain:

// ...
let call = fungible::Operation::Transfer {
    owner,
    amount,
    target_account,
};
// ...
self.runtime
    .call_application(/* authenticated by owner */ true, fungible_id, &call);
This causes Fungible::execute_operation to be run, which will create a cross-chain message sending the amount 10 to the Pugecoin application instance on Bob's chain.

After the cross-application call returns, CrowdFunding::execute_operation continues to create another cross-chain message crowd_funding::Message::PledgeWithAccount, which informs the crowd-funding application on Bob's chain that the 10 tokens are meant for the campaign.

When Bob now adds a block to his chain that handles the two incoming messages, first Fungible::execute_message gets executed, and then CrowdFunding::execute_message. The latter makes another cross-application call to transfer the 10 tokens from Carol's account to the crowd-funding application's account (both on Bob's chain). That is successful because Carol does now have 10 tokens on this chain and she authenticated the transfer indirectly by signing her block. The crowd-funding application now makes a note in its application state on Bob's chain that Carol has pledged 10 Pugecoin tokens.

References
For the complete code, please take a look at the crowd-funding and the fungible application contracts in the examples folder in linera-protocol.

The implementation of the Runtime made available to contracts is defined in this 

Using Data Blobs
Some applications may want to use static assets, like images or other data: e.g. the non-fungible example application implements NFTs, and each NFT has an associated image.

Data blobs are pieces of binary data that, once published on any chain, can be used on all chains. What format they are in and what they are used for is determined by the application(s) that read(s) them.

You can use the linera publish-data-blob command to publish the contents of a file, as an operation in a block on one of your chains. This will print the ID of the new blob, including its hash. Alternatively, you can run linera service and use the publishDataBlob GraphQL mutation.

Applications can now use runtime.read_data_blob(blob_hash) to read the blob. This works on any chain, not only the one that published it. The first time your client executes a block reading a blob, it will download the blob from the validators if it doesn't already have it locally.

In the case of the NFT app, it is only the service, not the contract, that actually uses the blob data to display it as an image in the frontend. But we still want to make sure that the user has the image locally as soon as they receive an NFT, even if they don't view it yet. This can be achieved by calling runtime.assert_data_blob_exists(blob_hash) in the contract: It will make sure the data is available, without actually loading it.

For the complete code please take a look at the non-fungible contract and service.


Printing Logs from an Application
Applications can use the log crate to print log messages with different levels of importance. Log messages are useful during development, but they may also be useful for end users. By default the linera service command will log the messages from an application if they are of the "info" importance level or higher (briefly, log::info!, log::warn! and log::error!).

During development it is often useful to log messages of lower importance (such as log::debug! and log::trace!). To enable them, the RUST_LOG environment variable must be set before running linera service. The example below enables trace level messages from applications and enables warning level messages from other parts of the linera binary:

export RUST_LOG="warn,linera_execution::wasm=trace"



Machine Learning on Linera
The Linera application contract / service split allows for securely and efficiently running machine learning models on the edge.

The application's contract retrieves the correct model with all the correctness guarantees enforced by the consensus algorithm, while the client performs inference off-chain, in the un-metered service. Since the service is running on the user's own hardware, it can be implicitly trusted.

Guidelines
The existing examples use the candle framework by Hugging Face as the underlying ML framework.

candle is a minimalist ML framework for Rust with a focus on performance and usability. It also compiles to Wasm and has great support for Wasm both in and outside the browser. Check candle's examples for inspiration on the types of models which are supported.

Getting started
To add ML capabilities to your existing Linera project, you'll need to add the candle-core, getrandom, rand and tokenizers dependencies to your Linera project:

candle-core = "0.4.1"
getrandom = { version = "0.2.12", default-features = false, features = ["custom"] }
rand = "0.8.5"
Optionally, to run Large Language Models, you'll also need the candle-transformers and transformers crate:

candle-transformers = "0.4.1"
tokenizers = { git = "https://github.com/christos-h/tokenizers", default-features = false, features = ["unstable_wasm"] }
Providing randomness
ML frameworks use random numbers to perform inference. Linera services run in a Wasm VM which does not have access to the OS Rng. For this reason, we need to manually seed RNG used by candle. We do this by writing a custom getrandom.

Create a file under src/random.rs and add the following:

use std::sync::{Mutex, OnceLock};

use rand::{rngs::StdRng, Rng, SeedableRng};

static RNG: OnceLock<Mutex<StdRng>> = OnceLock::new();

fn custom_getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    let seed = [0u8; 32];
    RNG.get_or_init(|| Mutex::new(StdRng::from_seed(seed)))
        .lock()
        .expect("failed to get RNG lock")
        .fill(buf);
    Ok(())
}

getrandom::register_custom_getrandom!(custom_getrandom);
This will enable candle and any other crates which rely on getrandom access to a deterministic RNG. If deterministic behaviour is not desired, the System API can be used to seed the RNG from a timestamp.

Loading the model into the service
Models cannot currently be saved on-chain; for more information see the Limitations below.

To perform model inference, the model must be loaded into the service. To do this we'll use the fetch_url API when a query is made against the service:

impl Service for MyService {
    async fn handle_query(&self, request: Request) -> Response {
        // do some stuff here
        let raw_weights = self.runtime.fetch_url("https://my-model-provider.com/model.bin");
        // do more stuff here
    }
}
This can be served from a local webserver or pulled directly from a model provider such as Hugging Face.

At this point we have the raw bytes which correspond to the models and tokenizer. candle supports multiple formats for storing model weights, both quantized and not (gguf, ggml, safetensors, etc.).

Depending on the model format that you're using, candle exposes convenience functions to convert the bytes into a typed struct which can then be used to perform inference. Below is an example for a non-quantized Llama 2 model:

    fn load_llama_model(cursor: &mut Cursor<Vec<u8>>) -> Result<(Llama, Cache), candle_core::Error> {
        let config = llama2_c::Config::from_reader(cursor)?;
        let weights =
            llama2_c_weights::TransformerWeights::from_reader(cursor, &config, &Device::Cpu)?;
        let vb = weights.var_builder(&config, &Device::Cpu)?;
        let cache = llama2_c::Cache::new(true, &config, vb.pp("rot"))?;
        let llama = Llama::load(vb, config.clone())?;
        Ok((llama, cache))
    }
Inference
Performing inference using candle is not a 'one-size-fits-all' process. Different models require different logic to perform inference so the specifics of how to perform inference are beyond the scope of this document.

Luckily, there are multiple examples which can be used as guidelines on how to perform inference in Wasm:

Llm Stories
Generative NFTs
Candle Wasm Examples
Limitations
Hardware acceleration
Although SIMD instructions are supported by the service runtime, general purpose GPU hardware acceleration is not currently supported. Therefore, performance in local model inference is degraded for larger models.

On-chain models
Due to block-size constraints, models need to be stored off-chain until the introduction of the Blob API. The Blob API will enable large binary blobs to be stored on-chain, the correctness and availability of which is guaranteed by the validators.

Maximum model size
The maximum size of a model which can be loaded into an application's service is currently constrained by:

The addressable memory of the service's Wasm runtime being 4 GiB.
Not being able to load models directly to the GPU.
It is recommended that smaller models (50 MB - 100 MB) are used at current state of development.


