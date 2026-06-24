def fetch_profile(channel):
    stub = user_pb2_grpc.UserServiceStub(channel)
    return stub.GetProfile()
