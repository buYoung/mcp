from tariff_extras import *
from tariff_table import with_tax as taxed


# The import binds `price` inside this function only.
def listed(amount):
    from tariff_table import price

    return price(amount)


# Neither this function nor the module binds `price`, so Python raises NameError here.
def unlisted(amount):
    return price(amount)


# The module binds `taxed` after the star import, so the star import cannot rebind it.
def gross(amount):
    return taxed(amount)
