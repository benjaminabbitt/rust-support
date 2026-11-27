# Messages for the crate's errors. Each id is an error's code without the
# crate's name, which the build prefixes (coded_diagnostic::build); the
# variables are its args.

no-version = the document has no version under the root key { $key }
bad-version = the version under the root key { $key } is not a non-negative integer: { $found }
root-not-a-mapping = the document's root is not a mapping, so it has no root key to read a version from
too-new = the document is version { $found }, newer than the { $current } this build reads
too-old = the document is version { $found }, older than the { $oldest } this build can still migrate
not-a-downgrade = a downgrade goes from { $from } to a lower version, not to { $to }
no-back = no step goes back from version { $from }
step-failed = the step from version { $from } to { $to } failed: { $cause }
round-trip = stepping version { $version } forward and back did not give the object back
wrong-type = the object is not a { $expected }
undecodable = the bytes of version { $version } do not decode: { $ext_err_txt }
registry-duplicate = the { $direction } step from version { $from } is registered twice
registry-out-of-range = the { $direction } step from version { $from } is outside the versions from { $oldest } to { $current }
registry-gap = there is no forward step from version { $from }
registry-back-without-forward = the back step from version { $from } has no forward step to undo
registry-inverted = the oldest version { $oldest } is newer than the current version { $current }
registry-type-mismatch = the forward step from version { $from } does not make the type the next version takes
registry-not-current = the last forward step, from version { $from }, does not make the registry's current type
registry-back-type-mismatch = the back step from version { $from } does not go between the types of its two versions
no-loader = the form has no loader for version { $version }
