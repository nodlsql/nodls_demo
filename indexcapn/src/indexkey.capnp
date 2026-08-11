@0xcb4c86144954cf2c;

# Composite key, no duplica
struct UniqIndexKey {
  idValue @0 :UInt32;
  cmpts @1 :List(KeyComponent);
}

# Composite key, allow duplica
struct DupIndexKey {
  dupIdValue @0 :DupIdUnion;
  cmpts @1 :List(KeyComponent);
}

struct DupIdUnion {
  union {
    uniqIdValue @0 :UInt32;
    ovfIdValue @1 :UInt32;         # overflow in sorted list
    listIdValue @2 :List(UInt32);
  }
}

struct DecimalValue {
  scale @0 :Int32;
  number @1 :Int64;
}

# Key value pair
struct KeyComponent {
  union {
    nullCpt @0 :Void;
    noneCpt @1 :Void;
    boolCpt @2 :Bool;
    strCpt @3 :Text;
    int64Cpt @4 :Int64;
    decimalCpt @5 :DecimalValue;
  }
}

# Use same page definition for node and leaf to avoid code duplication
struct IndexPage {
  union {
    uniqKeyEntries @0 :List(UniqIndexKey);    # no duplica entries
    dupKeyEntries @1 :List(DupIndexKey);  # allow key duplica entries
  }
}