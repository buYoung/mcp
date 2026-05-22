import { describe, expect, it } from "vitest";
import { layerZeroCheck, layerZeroCheckFromRawInput } from "../src/acp/layer-zero.js";

function fromArgv(argv: readonly string[]) {
    return { argv, paths: [] };
}

describe("layerZeroCheck — destructive_fs", () => {
    it("blocks rm -rf /", () => {
        const hit = layerZeroCheck(fromArgv(["rm", "-rf", "/"]));
        expect(hit?.category).toBe("destructive_fs");
        expect(hit?.code).toBe("DESTRUCTIVE_FS_RM_RF_ROOT");
    });

    it("blocks rm -rf ~ and $HOME", () => {
        expect(layerZeroCheck(fromArgv(["rm", "-rf", "~"]))?.code).toBe("DESTRUCTIVE_FS_RM_RF_ROOT");
        expect(layerZeroCheck(fromArgv(["rm", "-rf", "$HOME"]))?.code).toBe("DESTRUCTIVE_FS_RM_RF_ROOT");
    });

    it("blocks rm -fr / (combined flag order)", () => {
        expect(layerZeroCheck(fromArgv(["rm", "-fr", "/"]))?.code).toBe("DESTRUCTIVE_FS_RM_RF_ROOT");
    });

    it("blocks rm -r -f / (split flags)", () => {
        expect(layerZeroCheck(fromArgv(["rm", "-r", "-f", "/"]))?.code).toBe("DESTRUCTIVE_FS_RM_RF_ROOT");
    });

    it("allows scoped rm -rf inside cwd", () => {
        expect(layerZeroCheck(fromArgv(["rm", "-rf", "build"]))).toBeNull();
    });

    it("blocks find ... -delete", () => {
        expect(layerZeroCheck(fromArgv(["find", ".", "-name", "*.log", "-delete"]))?.code).toBe(
            "DESTRUCTIVE_FS_FIND_DELETE",
        );
    });

    it("blocks find ... -exec rm", () => {
        expect(layerZeroCheck(fromArgv(["find", ".", "-exec", "rm", "{}", ";"]))?.code).toBe(
            "DESTRUCTIVE_FS_FIND_EXEC_RM",
        );
    });

    it("blocks dd of=/dev/sda", () => {
        expect(layerZeroCheck(fromArgv(["dd", "if=/dev/zero", "of=/dev/sda"]))?.code).toBe("DESTRUCTIVE_FS_DD_DEVICE");
    });

    it("blocks mkfs.ext4 and diskutil eraseDisk", () => {
        expect(layerZeroCheck(fromArgv(["mkfs.ext4", "/dev/sda1"]))?.code).toBe("DESTRUCTIVE_FS_FORMAT");
        expect(layerZeroCheck(fromArgv(["diskutil", "eraseDisk", "JHFS+", "x", "disk2"]))?.code).toBe(
            "DESTRUCTIVE_FS_FORMAT",
        );
    });

    it("blocks shred / wipe / srm", () => {
        expect(layerZeroCheck(fromArgv(["shred", "secret"]))?.code).toBe("DESTRUCTIVE_FS_SECURE_DELETE");
    });
});

describe("layerZeroCheck — destructive_git", () => {
    it("blocks git push --force", () => {
        expect(layerZeroCheck(fromArgv(["git", "push", "--force"]))?.code).toBe("DESTRUCTIVE_GIT_PUSH_FORCE");
        expect(layerZeroCheck(fromArgv(["git", "push", "-f"]))?.code).toBe("DESTRUCTIVE_GIT_PUSH_FORCE");
        expect(layerZeroCheck(fromArgv(["git", "push", "--force-with-lease=main"]))?.code).toBe(
            "DESTRUCTIVE_GIT_PUSH_FORCE",
        );
    });

    it("blocks git push --mirror / --delete / :refspec", () => {
        expect(layerZeroCheck(fromArgv(["git", "push", "--mirror"]))?.code).toBe("DESTRUCTIVE_GIT_PUSH_MIRROR");
        expect(layerZeroCheck(fromArgv(["git", "push", "origin", "--delete", "main"]))?.code).toBe(
            "DESTRUCTIVE_GIT_PUSH_DELETE",
        );
        expect(layerZeroCheck(fromArgv(["git", "push", "origin", ":main"]))?.code).toBe(
            "DESTRUCTIVE_GIT_PUSH_DELETE_REFSPEC",
        );
    });

    it("blocks git reset --hard", () => {
        expect(layerZeroCheck(fromArgv(["git", "reset", "--hard", "HEAD~3"]))?.code).toBe("DESTRUCTIVE_GIT_RESET_HARD");
    });

    it("blocks --no-verify on any git subcommand", () => {
        expect(layerZeroCheck(fromArgv(["git", "commit", "--no-verify", "-m", "x"]))?.code).toBe(
            "DESTRUCTIVE_GIT_NO_VERIFY",
        );
    });

    it("blocks git filter-branch / filter-repo / gc --prune=now", () => {
        expect(layerZeroCheck(fromArgv(["git", "filter-branch"]))?.code).toBe("DESTRUCTIVE_GIT_FILTER");
        expect(layerZeroCheck(fromArgv(["git", "gc", "--prune=now"]))?.code).toBe("DESTRUCTIVE_GIT_GC_PRUNE");
    });

    it("blocks writes to .git/hooks/** and .husky/**", () => {
        const hookHit = layerZeroCheck({ argv: [], paths: ["/repo/.git/hooks/pre-commit"] });
        expect(hookHit?.code).toBe("DESTRUCTIVE_GIT_HOOKS_WRITE");
        const huskyHit = layerZeroCheck({ argv: [], paths: ["/repo/.husky/pre-push"] });
        expect(huskyHit?.code).toBe("DESTRUCTIVE_GIT_HUSKY_WRITE");
    });

    it("does not block plain git push / status", () => {
        expect(layerZeroCheck(fromArgv(["git", "push", "origin", "feature/x"]))).toBeNull();
        expect(layerZeroCheck(fromArgv(["git", "status"]))).toBeNull();
    });
});

describe("layerZeroCheck — system_modification", () => {
    it("blocks sudo / su / doas", () => {
        expect(layerZeroCheck(fromArgv(["sudo", "ls"]))?.code).toBe("SYSTEM_PRIVILEGE_ESCALATION");
        expect(layerZeroCheck(fromArgv(["su", "-"]))?.code).toBe("SYSTEM_PRIVILEGE_ESCALATION");
    });

    it("blocks package managers", () => {
        expect(layerZeroCheck(fromArgv(["apt-get", "install", "vim"]))?.code).toBe("SYSTEM_PACKAGE_MANAGER");
        expect(layerZeroCheck(fromArgv(["brew", "install", "jq"]))?.code).toBe("SYSTEM_PACKAGE_MANAGER");
    });

    it("blocks global npm/pnpm/yarn installs", () => {
        expect(layerZeroCheck(fromArgv(["npm", "i", "-g", "pkg"]))?.code).toBe("SYSTEM_NPM_GLOBAL_INSTALL");
        expect(layerZeroCheck(fromArgv(["pnpm", "add", "-g", "pkg"]))?.code).toBe("SYSTEM_NPM_GLOBAL_INSTALL");
        expect(layerZeroCheck(fromArgv(["yarn", "global", "add", "pkg"]))?.code).toBe("SYSTEM_NPM_GLOBAL_INSTALL");
    });

    it("blocks pipx / pip --user / cargo install / go install pkg@", () => {
        expect(layerZeroCheck(fromArgv(["pipx", "install", "ruff"]))?.code).toBe("SYSTEM_PIPX_INSTALL");
        expect(layerZeroCheck(fromArgv(["pip", "install", "--user", "x"]))?.code).toBe("SYSTEM_PIP_USER");
        expect(layerZeroCheck(fromArgv(["cargo", "install", "ripgrep"]))?.code).toBe("SYSTEM_CARGO_INSTALL");
        expect(layerZeroCheck(fromArgv(["go", "install", "github.com/x/y@latest"]))?.code).toBe("SYSTEM_GO_INSTALL");
    });

    it("allows local npm/pnpm/yarn installs", () => {
        expect(layerZeroCheck(fromArgv(["npm", "install"]))).toBeNull();
        expect(layerZeroCheck(fromArgv(["pnpm", "install"]))).toBeNull();
    });
});

describe("layerZeroCheck — credential_changes", () => {
    it("blocks git config --global", () => {
        expect(layerZeroCheck(fromArgv(["git", "config", "--global", "user.email", "x@y"]))?.code).toBe(
            "CREDENTIAL_GIT_CONFIG_GLOBAL",
        );
    });

    it("blocks ssh-keygen / gh auth / aws configure / gcloud auth / docker login", () => {
        expect(layerZeroCheck(fromArgv(["ssh-keygen", "-t", "ed25519"]))?.code).toBe("CREDENTIAL_SSH_KEYGEN");
        expect(layerZeroCheck(fromArgv(["gh", "auth", "login"]))?.code).toBe("CREDENTIAL_GH_AUTH");
        expect(layerZeroCheck(fromArgv(["aws", "configure"]))?.code).toBe("CREDENTIAL_AWS_CONFIGURE");
        expect(layerZeroCheck(fromArgv(["gcloud", "auth", "login"]))?.code).toBe("CREDENTIAL_GCLOUD_AUTH");
        expect(layerZeroCheck(fromArgv(["docker", "login"]))?.code).toBe("CREDENTIAL_DOCKER_LOGIN");
    });

    it("allows git config --local", () => {
        expect(layerZeroCheck(fromArgv(["git", "config", "--local", "user.email", "x@y"]))).toBeNull();
    });
});

describe("layerZeroCheck — shell_evasion", () => {
    it("blocks bare eval", () => {
        expect(layerZeroCheck(fromArgv(["eval", "$cmd"]))?.code).toBe("SHELL_EVAL");
    });

    it("blocks interpreter one-liners with FS / network mutation", () => {
        expect(layerZeroCheck(fromArgv(["python", "-c", "import subprocess; subprocess.run('rm -rf /')"]))?.code).toBe(
            "SHELL_INTERPRETER_ONELINER",
        );
        expect(layerZeroCheck(fromArgv(["node", "-e", "require('child_process').exec('rm')"]))?.code).toBe(
            "SHELL_INTERPRETER_ONELINER",
        );
        expect(
            layerZeroCheck(fromArgv(["python3", "-c", "import urllib.request; urllib.request.urlopen(u)"]))?.code,
        ).toBe("SHELL_INTERPRETER_ONELINER");
        expect(layerZeroCheck(fromArgv(["perl", "-e", "print 1+1"]))).toBeNull();
    });

    it("allows interpreter one-liners without dangerous calls", () => {
        expect(layerZeroCheck(fromArgv(["python", "-c", "print(2+2)"]))).toBeNull();
        expect(layerZeroCheck(fromArgv(["node", "-e", "console.log(1)"]))).toBeNull();
    });

    it("blocks base64 -d", () => {
        expect(layerZeroCheck(fromArgv(["base64", "-d"]))?.code).toBe("SHELL_BASE64_DECODE");
    });
});

describe("layerZeroCheck — exfiltration", () => {
    it("blocks curl with --data / --form / -X POST", () => {
        expect(layerZeroCheck(fromArgv(["curl", "--data", "k=v", "https://x"]))?.code).toBe("EXFIL_CURL_DATA");
        expect(layerZeroCheck(fromArgv(["curl", "-F", "f=@a", "https://x"]))?.code).toBe("EXFIL_CURL_DATA");
        expect(layerZeroCheck(fromArgv(["curl", "-X", "POST", "https://x"]))?.code).toBe("EXFIL_CURL_WRITE_METHOD");
        expect(layerZeroCheck(fromArgv(["curl", "--request", "delete", "https://x"]))?.code).toBe(
            "EXFIL_CURL_WRITE_METHOD",
        );
    });

    it("allows plain curl GET", () => {
        expect(layerZeroCheck(fromArgv(["curl", "https://x"]))).toBeNull();
        expect(layerZeroCheck(fromArgv(["curl", "-X", "GET", "https://x"]))).toBeNull();
    });

    it("blocks wget --post-data / --post-file", () => {
        expect(layerZeroCheck(fromArgv(["wget", "--post-data=k=v", "https://x"]))?.code).toBe("EXFIL_WGET_POST");
    });

    it("blocks scp / nc / ncat", () => {
        expect(layerZeroCheck(fromArgv(["scp", "a", "user@host:/b"]))?.code).toBe("EXFIL_SCP");
        expect(layerZeroCheck(fromArgv(["nc", "host", "443"]))?.code).toBe("EXFIL_NETCAT");
        expect(layerZeroCheck(fromArgv(["ncat", "host", "443"]))?.code).toBe("EXFIL_NETCAT");
    });

    it("blocks rsync with host:path", () => {
        expect(layerZeroCheck(fromArgv(["rsync", "-av", "a/", "user@host:/dest"]))?.code).toBe("EXFIL_RSYNC_REMOTE");
    });

    it("allows local rsync", () => {
        expect(layerZeroCheck(fromArgv(["rsync", "-av", "a/", "b/"]))).toBeNull();
    });

    it("blocks ssh user@host", () => {
        expect(layerZeroCheck(fromArgv(["ssh", "user@host", "ls"]))?.code).toBe("EXFIL_SSH_REMOTE");
    });

    it("blocks URLs with embedded credentials", () => {
        expect(layerZeroCheck(fromArgv(["curl", "https://user:tok@example.com/x"]))?.code).toBe(
            "EXFIL_CREDENTIAL_IN_URL",
        );
        expect(layerZeroCheck(fromArgv(["git", "clone", "https://x:tok@github.com/o/r"]))?.code).toBe(
            "EXFIL_CREDENTIAL_IN_URL",
        );
    });
});

describe("layerZeroCheck — self_privilege_escalation", () => {
    it("blocks .claude/settings.json", () => {
        const hit = layerZeroCheck({ argv: [], paths: ["/repo/.claude/settings.json"] });
        expect(hit?.code).toBe("SELF_PRIV_CLAUDE_SETTINGS");
    });

    it("blocks ~/.claude/** generic", () => {
        const hit = layerZeroCheck({ argv: [], paths: ["/Users/x/.claude/projects/y.md"] });
        expect(hit?.code).toBe("SELF_PRIV_CLAUDE_DIR");
    });

    it("blocks shell rc files", () => {
        expect(layerZeroCheck({ argv: [], paths: ["/Users/x/.bashrc"] })?.code).toBe("SELF_PRIV_SHELL_RC");
        expect(layerZeroCheck({ argv: [], paths: ["/Users/x/.zshrc"] })?.code).toBe("SELF_PRIV_SHELL_RC");
    });

    it("blocks fish config and ~/.config/git", () => {
        expect(layerZeroCheck({ argv: [], paths: ["/Users/x/.config/fish/config.fish"] })?.code).toBe(
            "SELF_PRIV_FISH_CONFIG",
        );
        expect(layerZeroCheck({ argv: [], paths: ["/Users/x/.config/git/config"] })?.code).toBe("SELF_PRIV_GIT_CONFIG");
    });

    it("blocks .npmrc / pip.conf", () => {
        expect(layerZeroCheck({ argv: [], paths: ["/repo/.npmrc"] })?.code).toBe("SELF_PRIV_PACKAGE_CONFIG");
        expect(layerZeroCheck({ argv: [], paths: ["/etc/pip.conf"] })?.code).toBe("SELF_PRIV_PACKAGE_CONFIG");
    });
});

describe("layerZeroCheckFromRawInput", () => {
    it("unwraps shell wrappers before matching", () => {
        const hit = layerZeroCheckFromRawInput({ command: ["bash", "-c", "git push --force"] });
        expect(hit?.code).toBe("DESTRUCTIVE_GIT_PUSH_FORCE");
    });

    it("matches via string command", () => {
        const hit = layerZeroCheckFromRawInput({ command: "sudo rm -rf /" });
        expect(hit?.category).toBe("system_modification");
    });

    it("returns null for safe commands", () => {
        expect(layerZeroCheckFromRawInput({ command: ["ls", "-la"] })).toBeNull();
    });
});
