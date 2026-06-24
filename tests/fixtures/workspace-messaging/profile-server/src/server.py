class UserServiceServicer:
    def GetProfile(self, request, context):
        return None


def serve(server):
    user_pb2_grpc.add_UserServiceServicer_to_server(UserServiceServicer(), server)
