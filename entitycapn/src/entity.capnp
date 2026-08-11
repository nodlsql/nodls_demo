@0xf929f0e4a4b0afde;

# TBD - att lobj

struct RelPart {
  rels @0 :List(Rel);
}

struct Rel {
  rid @0 :UInt32;
  union {
    rSuccs @1 : List(UInt32);
    rSet @2 : RelSet;
    # TBD - paginated sorted list - slist @1 : SList;
  }
}

# Non-embedded set of Ids. Same rel id, different NS:
# . Rel NS
# . RelSet NS
# . (TBD) RelSList NS
# Duplicate these NS for inverse
struct RelSet {
  rsId @0 :UInt32;            # repeat individual rel id
  rsSuccs @1 :List(UInt32);
}