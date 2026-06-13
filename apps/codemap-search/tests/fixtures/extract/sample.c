/** Fixture exercising C branch-sensitive extraction. */

#define MAX_CONNECTIONS 16

/** A connection record. */
struct Connection {
    int fd;
};

/** Open a connection and return a status message. */
const char *connection_open(void) {
    const char *status = "connection opened";
    return status;
}

/**
 * @deprecated use connection_open instead
 */
const char *connection_open_legacy(void) {
    return "legacy";
}

/* A file-local helper (static is not exported). */
static int internal_count(void) {
    return 0;
}
