#if defined(_MSC_VER)
#pragma once
#endif

#ifndef HANIMMGR_H
#define HANIMMGR_H

#include "always.h"
#include "hash.h"
#include "wwstring.h"
#include "vector.h"

class HAnimClass;
class ChunkLoadClass;
class W3DExclusionListClass;

/*
** An entry for a table of anims not found, so we can quickly determine their loss
*/
class MissingAnimClass : public HashableClass {

public:
	MissingAnimClass( const char * name ) : Name( name ) {}
	virtual	~MissingAnimClass( void ) {}

	virtual	const char * Get_Key( void )	{ return Name;	}

private:
	StringClass	Name;

};

/*
	HAnimManagerClass

	This class is used to keep track of all of the motion data.
*/

class HAnimManagerClass
{

public:
	HAnimManagerClass(void);
	~HAnimManagerClass(void);

	int			 		Load_Anim(ChunkLoadClass & cload);
	HAnimClass *		Get_Anim(const char * name);
	HAnimClass *		Peek_Anim(const char * name);
	bool					Add_Anim(HAnimClass *new_anim);
	void			 		Free_All_Anims(void);
	void			 		Free_All_Anims_With_Exclusion_List(const W3DExclusionListClass & exclusion_list);
	void					Create_Asset_List(DynamicVectorClass<StringClass> & exclusion_list);

	void					Register_Missing( const char * name );
	bool					Is_Missing( const char * name );
	void					Reset_Missing( void );

private:
	int					Load_Compressed_Anim(ChunkLoadClass & cload);
	int					Load_Raw_Anim(ChunkLoadClass & cload);
	int					Load_Morph_Anim(ChunkLoadClass & cload);

	HashTableClass	*	AnimPtrTable;
	HashTableClass	*	MissingAnimTable;

	friend	class		HAnimManagerIterator;
};


/*
** An Iterator to get to all loaded HAnims in a HAnimManager
*/
class HAnimManagerIterator : public HashTableIteratorClass {
public:
	HAnimManagerIterator( HAnimManagerClass & manager ) : HashTableIteratorClass( *manager.AnimPtrTable ) {}
	HAnimClass * Get_Current_Anim( void );
};

#endif