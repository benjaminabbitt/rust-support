# Messages for the crate's errors. Each id is an error's code without the
# crate's name, which the build prefixes (coded_diagnostic::build); the
# variables are its args.

parse = the { $format } text does not parse: { $ext_err_txt }
not-round-trip = the object written as { $format } would not read back as it is
stamp-mismatch = the object written as { $format } would read as version { $found }, not the current { $current }: its type must carry its version
read = { $path } could not be read: { $ext_err_txt }
write = { $path } could not be written: { $ext_err_txt }
backup-exists = the backup { $path } is already there: it is never overwritten
backup = the backup { $path } could not be made: { $ext_err_txt }
changed-since-read = { $path } changed after it was read: nothing was written
migrate = { $path } does not migrate: { $cause }
