use std::fmt::Debug;
use std::future::Future;
use std::sync::Arc;

use hyper::Uri;
use tempfile::TempPath;
use tokio::net::UnixStream;
use tonic::transport::{Channel, Endpoint};
use tonic::{IntoRequest, Request, Response, Status};

use blockvisor_api::auth::token::jwt::Jwt;
use tracing::debug;

pub trait GrpcClient<T> {
    fn create(channel: Channel) -> Self;
}

grpc_clients! [
    api_key => ApiKey,
    auth => Auth,
    blockchain => Blockchain,
    blockchain_archive => BlockchainArchive,
    bundle => Bundle,
    command => Command,
    discovery => Discovery,
    host => Host,
    invitation => Invitation,
    kernel => Kernel,
    metrics => Metrics,
    node => Node,
    org => Org,
    subscription => Subscription,
    user => User
];

#[tonic::async_trait]
pub trait SocketRpc {
    fn input_socket(&self) -> Arc<TempPath>;

    async fn root_jwt(&self) -> Jwt;

    async fn admin_jwt(&self) -> Jwt;

    async fn member_jwt(&self) -> Jwt;

    /// Send a request without any authentication to the test server.
    ///
    /// All the functions that we want to test are of a similar type, because
    /// they are all generated by tonic.
    ///
    /// ## Examples
    /// Some examples in a central place here:
    ///
    /// ### Simple test
    /// ```rs
    /// type Service = AuthenticationService<Channel>;
    /// let test = TestServer::new().await;
    /// test.send(Service::login, your_login_request).await.unwrap();
    /// let status = test.send(Service::login, bad_login_request).await.unwrap_err();
    /// assert_eq!(status.code(), tonic::Code::Unauthenticated);
    /// ```
    ///
    /// ### Test for success
    /// ```rs
    /// type Service = AuthenticationService<Channel>;
    /// let test = TestServer::new().await;
    /// test.send(Service::refresh, req).await.unwrap();
    /// ```
    ///
    /// ### Generic params
    /// We have some generics going on here so lets break it down.
    ///
    /// The function that we want to test is of type `F`. Its signature is
    /// required to be `(&mut Client, Req) -> impl Future<Output =
    /// Result<Response<Resp>, Status>>`.
    ///
    /// We further restrict that `Req` must satisfy `impl IntoRequest<In>`. This
    /// means that `In` is the JSON structure that the requests take, `Req` is
    /// the type that the function takes that can be constructed from the `In`
    /// type, and `Resp` is the type that is returned on success.
    async fn send<F, In, Req, Resp, Client>(&self, f: F, req: Req) -> Result<Resp, Status>
    where
        F: for<'any> TestableFunction<'any, Request<In>, Response<Resp>, Client>,
        In: Send + Debug,
        Req: IntoRequest<In> + Send,
        Resp: Send + Debug,
        Client: GrpcClient<Channel> + Send + Debug + 'static,
    {
        self.send_request(f, req.into_request()).await
    }

    /// Sends the provided request to the provided function, just as `send`
    /// would do, but adds the provided token to the metadata of the request.
    /// The token is base64 encoded and prefixed with `"Bearer "`. This allows
    /// you to send custom authentication through the testing machinery, which
    /// is needed for stuff like testing auth.
    ///
    /// ## Examples
    /// Some examples to demonstrate how to make tests with this:
    ///
    /// ### Empty token
    /// ```rs
    /// type Service = SomeService<Channel>;
    /// let test = TestServer::new().await;
    /// let status = test.send(Service::some_endpoint, some_data, "").await.unwrap_err();
    /// assert_eq!(status.code(), tonic::Code::Unauthorized);
    /// ```
    async fn send_with<F, In, Req, Resp, Client>(
        &self,
        f: F,
        req: Req,
        token: &str,
    ) -> Result<Resp, Status>
    where
        F: for<'any> TestableFunction<'any, Request<In>, Response<Resp>, Client>,
        In: Send + Debug,
        Req: IntoRequest<In> + Send,
        Resp: Send + Debug,
        Client: GrpcClient<Channel> + Send + Debug + 'static,
    {
        let mut req = req.into_request();
        let auth_header = format!("Bearer {}", token).parse().unwrap();
        req.metadata_mut().insert("authorization", auth_header);
        self.send_request(f, req).await
    }

    /// Send a request with authentication as a blockjoy admin user.
    async fn send_root<F, In, Req, Resp, Client>(&self, f: F, req: Req) -> Result<Resp, Status>
    where
        F: for<'any> TestableFunction<'any, Request<In>, Response<Resp>, Client>,
        In: Send + Debug,
        Req: IntoRequest<In> + Send,
        Resp: Send + Debug,
        Client: GrpcClient<Channel> + Send + Debug + 'static,
    {
        let jwt = self.root_jwt().await;
        self.send_with(f, req, &jwt).await
    }

    /// Send a request with authentication as a seed org admin.
    async fn send_admin<F, In, Req, Resp, Client>(&self, f: F, req: Req) -> Result<Resp, Status>
    where
        F: for<'any> TestableFunction<'any, Request<In>, Response<Resp>, Client>,
        In: Send + Debug,
        Req: IntoRequest<In> + Send,
        Resp: Send + Debug,
        Client: GrpcClient<Channel> + Send + Debug + 'static,
    {
        let jwt = self.admin_jwt().await;
        self.send_with(f, req, &jwt).await
    }

    /// Send a request with authentication as a seed org member.
    async fn send_member<F, In, Req, Resp, Client>(&self, f: F, req: Req) -> Result<Resp, Status>
    where
        F: for<'any> TestableFunction<'any, Request<In>, Response<Resp>, Client>,
        In: Send + Debug,
        Req: IntoRequest<In> + Send,
        Resp: Send + Debug,
        Client: GrpcClient<Channel> + Send + Debug + 'static,
    {
        let jwt = self.member_jwt().await;
        self.send_with(f, req, &jwt).await
    }

    async fn send_request<F, In, Resp, Client>(
        &self,
        f: F,
        req: Request<In>,
    ) -> Result<Resp, Status>
    where
        F: for<'any> TestableFunction<'any, Request<In>, Response<Resp>, Client>,
        In: Send + Debug,
        Resp: Send + Debug,
        Client: GrpcClient<Channel> + Send + Debug + 'static,
    {
        let socket = self.input_socket();
        let channel = Endpoint::try_from("http://any.url")
            .unwrap()
            .connect_with_connector(tower::service_fn(move |_: Uri| {
                let socket = socket.clone();
                async move { UnixStream::connect(&*socket).await }
            }))
            .await
            .unwrap();
        let mut client = Client::create(channel);

        debug!("{:?}", req.get_ref());
        f(&mut client, req).await.map(|resp| {
            let resp = resp.into_inner();
            debug!("{:?}", &resp);
            resp
        })
    }
}

/// This is a client function that we can run through the test machinery. This
/// contains a _lot_ of generics so lets break it down:
///
/// 1. `'c`: This is the lifetime of the client. We restrict the lifetime of the
///    generated by the tested function to be at most `'c`, because that future
///    must borrow the client to make progress.
///
/// 2. `Req`: This is some type that implements `IntoRequest<In>`, where `In`
///    typically a struct implementing `Deserialize`.
///
/// 3. `Resp`: This is the type of data that the function returns. Usually a
///    struct (sometimes an enum) that implements `Serialize`.
///
/// 4. `Client`: This is the client struct that is used to query the server.
///    These are generated by `tonic` from the proto files, and are generic over
///    the transport layer. An example of what could go here is
///    `AuthenticationServiceClient<Channel>`. The `send` functions require that
///    this type implements `GrpcClient`.
pub trait TestableFunction<'c, Req, Resp, Client>:
    Fn(&'c mut Client, Req) -> Self::Fut + Send + Sync
where
    Req: Send,
    Resp: Send,
    Client: 'static,
{
    type Fut: Future<Output = Result<Resp, Status>> + Send + 'c;
}

/// Implement our test function trait for all functions of the right signature.
impl<'c, F, Fut, Req, Resp, Client> TestableFunction<'c, Req, Resp, Client> for F
where
    F: Fn(&'c mut Client, Req) -> Fut + Send + Sync,
    Req: Send,
    Resp: Send,
    Fut: Future<Output = Result<Resp, Status>> + Send + 'c,
    Client: 'static,
{
    type Fut = Fut;
}
