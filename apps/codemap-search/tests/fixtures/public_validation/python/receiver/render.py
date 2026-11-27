from string import Template


# The only indexed `substitute`, a module-level function.
def substitute(text):
    return text.strip()


# The receiver is declared with a type that this repository does not declare.
def render(template: Template, name: str) -> str:
    return template.substitute(name=name)


# The receiver type is not written here, so this call stays unconfirmed.
def render_any(template, name):
    return template.substitute(name=name)
