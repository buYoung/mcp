typedef void (*Callback)(void);
static Callback current, other;
void install(Callback f, Callback g) {
    current = f; /* @S1 */
    other = g; /* @S2 */
}
void fire(void) { current(); } /* @I1 */
void shadow(Callback current) { current(); } /* @I2 */
