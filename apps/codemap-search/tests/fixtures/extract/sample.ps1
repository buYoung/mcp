Import-Module 'Demo.Helper'

class Worker {
    [string] $Name = 'powershell'

    [void] Run() {
        Start-Worker
    }
}
