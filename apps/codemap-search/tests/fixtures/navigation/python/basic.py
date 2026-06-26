from services.user import UserService
import os


def run():
    user = UserService()
    user.save()
    print(os.getcwd())


class Controller:
    def handle(self):
        run()
