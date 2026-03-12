#if defined(_MSC_VER)
#pragma once
#endif

#ifndef W3DEXCLUSIONLIST_H
#define W3DEXCLUSIONLIST_H

#include "always.h"
#include "vector.h"
#include "wwstring.h"
#include "hashtemplate.h"

class PrototypeClass;
class HTreeClass;
class HAnimClass;


/**
** W3DExclusionListClass
** This class ecapsulates an "exclusion list" which the asset manager and related classes use
** to filter what resources get released.  This is useful between level loads for example.  
** The Is_Excluded function uses w3d naming convention assumptions to determine whether the given
** asset name is in the list or is a child of something in the list.
*/

class W3DExclusionListClass
{
public:
	W3DExclusionListClass(const DynamicVectorClass<StringClass> & names);
	~W3DExclusionListClass(void);
	
	bool	Is_Excluded(PrototypeClass * proto) const;
	bool	Is_Excluded(HTreeClass * htree) const;
	bool	Is_Excluded(HAnimClass * hanim) const;
	
	bool	Is_Excluded(const char * root_name) const;

protected:


	const DynamicVectorClass<StringClass> &	Names;
	HashTemplateClass<StringClass,int>			NameHash;
};



#endif //EXCLUSIONLIST_H