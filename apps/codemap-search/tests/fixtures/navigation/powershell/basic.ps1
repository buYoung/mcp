Import-Module 'Demo.Helper'

class PowerShellFlow {
    [void] TargetPowerShell() {}
    [void] CallerPowerShell() {
        TargetPowerShell
    }
}
