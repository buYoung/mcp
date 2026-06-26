import com.example.User as UserAlias

class User {
    fun save() {}
}

interface Saver {
    fun save()
}

fun run() {
    val user: User = UserAlias()
    user.save()
}
