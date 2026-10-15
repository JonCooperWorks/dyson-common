# BIN table

`bins.tsv.gz` is what the payment detector (`src/payment.rs`) names a card's
issuer from: the first six digits of a card number (its BIN), and for each
the scheme, whether it is a credit or a debit card, its category, the
issuing bank and the bank's country. It is compiled into the crate when the
`payment` feature is on; no card number, or part of one, is ever looked up
anywhere else.

It is the BIN List Data of <https://github.com/venelinkochev/bin-list-data>
(`bin-list-data.csv` at commit `023a4f68ab1b5d7edd45f03acb132f88a49db96a`,
374,788 BINs), made available under the Creative Commons Attribution 4.0
International License (<https://creativecommons.org/licenses/by/4.0/>).
Changed here: the issuer's telephone number and site, the three-letter
country code and the country's name are left out, the rows are sorted by
BIN, and the columns are separated by tabs:

    BIN  Brand  Type  Category  Issuer  isoCode2

To take a newer one, write the same six columns from the newer CSV, sorted,
and gzip them.
