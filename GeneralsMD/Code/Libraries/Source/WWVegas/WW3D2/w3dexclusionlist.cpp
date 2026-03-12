#include "w3dexclusionlist.h"
#include "proto.h"
#include "htree.h"
#include "hanim.h"


W3DExclusionListClass::W3DExclusionListClass(const DynamicVectorClass<StringClass> & names) : 
	Names(names) 
{
	for (int i=0; i<Names.Count(); i++) {
		NameHash.Insert(Names[i],i);
	}
}

W3DExclusionListClass::~W3DExclusionListClass(void) 
{ 
	NameHash.Remove_All();
}

bool	W3DExclusionListClass::Is_Excluded(PrototypeClass * proto) const
{
	StringClass copy = proto->Get_Name();
	char * root_name = copy.Peek_Buffer();
	
	// don't preserve munged prototypes
	if (strchr(root_name,'#') != NULL) {
		return false;
	}

	// chop off the sub-object name if present (
	char * tmp = strchr(root_name,'.');
	if (tmp != NULL) {
		*tmp = 0;
	}
	
	return Is_Excluded(root_name);
}


bool	W3DExclusionListClass::Is_Excluded(HTreeClass * htree) const
{
	// plain old name...
	return Is_Excluded(htree->Get_Name());
}


bool	W3DExclusionListClass::Is_Excluded(HAnimClass * hanim) const
{
	// For HAnims, the name to check is the one trailing the '.'
	StringClass copy = hanim->Get_Name();
	char * root_name = copy.Peek_Buffer();

	char * tmp = strchr(root_name,'.');
	if (tmp) {
		return Is_Excluded(tmp + 1);
	} else { 
		return false;  // should never happen...
	}
}


bool W3DExclusionListClass::Is_Excluded(const char * root_name) const
{
	return NameHash.Exists(StringClass(root_name));	
}


