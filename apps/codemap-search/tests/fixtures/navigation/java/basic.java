import com.example.UserService;

class Controller {
    void run() {
        UserService userService = new UserService();
        userService.save();
    }
}

class UserService {
    void save() {}
}
