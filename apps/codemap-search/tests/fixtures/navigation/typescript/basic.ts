import Cache from "./cache";
import { save as persist, UserService } from "./user";

export function run() {
    const user: UserService = new UserService();
    user.save();
    persist(user);
    Cache.flush();
}

export class Controller {
    handle() {
        run();
    }
}
