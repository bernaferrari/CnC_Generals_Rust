#include "hanimmgr.h"
#include <string.h>
#include "hanim.h"
#include "hrawanim.h"
#include "hcanim.h"
#include "hmorphanim.h"
#include "chunkio.h"
#include "wwmemlog.h"
#include "w3dexclusionlist.h"
#include "animatedsoundmgr.h"


/*********************************************************************************************** 
 * HAnimManagerClass::HAnimManagerClass -- constructor                                         * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   08/11/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
HAnimManagerClass::HAnimManagerClass(void) 
{
	// Create the hash tables
	AnimPtrTable = W3DNEW HashTableClass( 2048 );
	MissingAnimTable = W3DNEW HashTableClass( 2048 );
}


/*********************************************************************************************** 
 * HAnimManagerClass::~HAnimManagerClass -- destructor                                         * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   08/11/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
HAnimManagerClass::~HAnimManagerClass(void)
{
	Free_All_Anims();
	Reset_Missing();	// Jani: Deleting missing animations as well

	delete AnimPtrTable;
	AnimPtrTable = NULL;

	Reset_Missing();
	delete MissingAnimTable;
	MissingAnimTable = NULL;
}


/*********************************************************************************************** 
 * HAnimManagerClass::Load_Anim -- loads a set of motion data from a file                      * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   08/11/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
int HAnimManagerClass::Load_Anim(ChunkLoadClass & cload)
{
	WWMEMLOG(MEM_ANIMATION);

	switch (cload.Cur_Chunk_ID()) 
	{
	case W3D_CHUNK_ANIMATION:
		return Load_Raw_Anim(cload);
		break;

	case W3D_CHUNK_COMPRESSED_ANIMATION:
		return Load_Compressed_Anim(cload);
		break;

	case W3D_CHUNK_MORPH_ANIMATION:
		return Load_Morph_Anim(cload);
		break;
	}

	return 0;
}


/***********************************************************************************************
 * HAnimManagerClass::Load_Morph_Anim -- Load a HMorphAnimClass										  *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   5/23/2000  pds : Created.                                                                 *
 *=============================================================================================*/
int HAnimManagerClass::Load_Morph_Anim(ChunkLoadClass & cload)
{
	HMorphAnimClass * newanim = W3DNEW HMorphAnimClass;

	if (newanim == NULL) {
		goto Error;
	}

	SET_REF_OWNER( newanim );

	if (newanim->Load_W3D(cload) != HMorphAnimClass::OK) {
		// load failed!
		newanim->Release_Ref();
		goto Error;
	} else if (Peek_Anim(newanim->Get_Name()) != NULL) {
		// duplicate exists!
		newanim->Release_Ref();	// Release the one we just loaded
		goto Error;
	} else {
		Add_Anim( newanim );
		newanim->Release_Ref();
	}

	return 0;

Error:

	return 1;
}


/***********************************************************************************************
 * HAnimManagerClass::Load_Raw_Anim -- Load a raw anim                                         *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   5/23/2000  gth : Created.                                                                 *
 *=============================================================================================*/
int HAnimManagerClass::Load_Raw_Anim(ChunkLoadClass & cload)
{
	HRawAnimClass * newanim = W3DNEW HRawAnimClass;

	if (newanim == NULL) {
		goto Error;
	}

	SET_REF_OWNER( newanim );

	if (newanim->Load_W3D(cload) != HRawAnimClass::OK) {
		// load failed!
		newanim->Release_Ref();
		goto Error;
	} else if (Peek_Anim(newanim->Get_Name()) != NULL) {
		// duplicate exists!
		newanim->Release_Ref();	// Release the one we just loaded
		goto Error;
	} else {
		Add_Anim( newanim );
		newanim->Release_Ref();
	}

	return 0;

Error:

	return 1;
}


/***********************************************************************************************
 * HAnimManagerClass::Load_Compressed_Anim -- load a compressed animation                      *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   5/23/2000  gth : Created.                                                                 *
 *=============================================================================================*/
int HAnimManagerClass::Load_Compressed_Anim(ChunkLoadClass & cload)
{
	HCompressedAnimClass * newanim = W3DNEW HCompressedAnimClass;

	if (newanim == NULL) {
		goto Error;
	}

	SET_REF_OWNER( newanim );

	if (newanim->Load_W3D(cload) != HCompressedAnimClass::OK) {
		// load failed!
		newanim->Release_Ref();
		goto Error;
	} else if (Peek_Anim(newanim->Get_Name()) != NULL) {
		// duplicate exists!
		newanim->Release_Ref();	// Release the one we just loaded
		goto Error;
	} else {
		Add_Anim( newanim );
		newanim->Release_Ref();
	}

	return 0;

Error:

	return 1;
}

/*********************************************************************************************** 
 * HAnimManagerClass::Peek_Anim -- returns a pointer to the specified animation data            * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   08/11/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
HAnimClass * HAnimManagerClass::Peek_Anim(const char * name)
{
	return (HAnimClass*)AnimPtrTable->Find( name );
}


/*********************************************************************************************** 
 * HAnimManagerClass::Get_Anim -- returns a pointer to the specified animation data            * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   08/11/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
HAnimClass * HAnimManagerClass::Get_Anim(const char * name)
{	
	HAnimClass * anim = Peek_Anim( name );
	if ( anim != NULL ) {
		anim->Add_Ref();
	}
	return anim;
}


/*********************************************************************************************** 
 * HAnimManagerClass::Free_All_Anims -- de-allocate all currently loaded animations            * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   08/11/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
void HAnimManagerClass::Free_All_Anims(void)
{
	// Make an iterator, and release all ptrs
	HAnimManagerIterator it( *this );
	for( it.First(); !it.Is_Done(); it.Next() ) {
		HAnimClass *anim = it.Get_Current_Anim();
		anim->Release_Ref();
	}

	// Then clear the table
	AnimPtrTable->Reset();
}
	
/*********************************************************************************************** 
 * HAnimManagerClass::Free_All_Anims_With_Exclusion_List -- release animations not in the list * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   12/12/2002 GH  : Created.                                                                 * 
 *=============================================================================================*/
void HAnimManagerClass::Free_All_Anims_With_Exclusion_List(const W3DExclusionListClass & exclusion_list)
{
	// Remove and Release_Ref any animation not in the exclusion list.
	HAnimManagerIterator it( *this );
	for( it.First(); !it.Is_Done(); it.Next() ) {
		HAnimClass *anim = it.Get_Current_Anim();

		if ((anim->Num_Refs() == 1) && (exclusion_list.Is_Excluded(anim) == false)) {
			//WWDEBUG_SAY(("deleting HAnim %s\n",anim->Get_Name()));
			AnimPtrTable->Remove(anim);
			anim->Release_Ref();
		}
		//else
		//{
		//	WWDEBUG_SAY(("keeping HAnim %s (ref %d)\n",anim->Get_Name(),anim->Num_Refs()));
		//}
	}
}


/*********************************************************************************************** 
 * HAnimManagerClass::Create_Asset_List -- Create a list of the W3D files that are loaded      * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   12/12/2002 GH  : Created.                                                                 * 
 *=============================================================================================*/
void HAnimManagerClass::Create_Asset_List(DynamicVectorClass<StringClass> & exclusion_list)
{
	HAnimManagerIterator it( *this );
	for( it.First(); !it.Is_Done(); it.Next() ) {
		HAnimClass *anim = it.Get_Current_Anim();

		// File that this anim came from should be the name after the '.'
		// Anims are named in the format: <skeleton>.<animname>
		const char * anim_name = anim->Get_Name();
		char * filename = strchr(anim_name,'.');
		if (filename != NULL) {	
			exclusion_list.Add(StringClass(filename+1));
		}
	}
}


/*********************************************************************************************** 
 * HAnimManagerClass::Add_Anim -- Adds an externally created animation to the manager			  *
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   05/31/2000 PDS  : Created.                                                                * 
 *=============================================================================================*/
bool HAnimManagerClass::Add_Anim(HAnimClass *new_anim)
{
	WWASSERT (new_anim != NULL);

	// Increment the refcount on the W3DNEW animation and add it to our table.
	new_anim->Add_Ref ();
	AnimPtrTable->Add( new_anim );
	
	return true;
}


/*
** Missing Anims
**
** The idea here, allow the system to register which anims are determined to be missing
** so that if they are asked for again, we can quickly return NULL, without searching the
** disk again.
*/
void	HAnimManagerClass::Register_Missing( const char * name )
{
	MissingAnimTable->Add( W3DNEW MissingAnimClass( name ) );
}

bool	HAnimManagerClass::Is_Missing( const char * name )
{
	return ( MissingAnimTable->Find( name ) != NULL );
}

void	HAnimManagerClass::Reset_Missing( void )
{
	// Make an iterator, and release all ptrs
	HashTableIteratorClass it( *MissingAnimTable );
	for( it.First(); !it.Is_Done(); it.Next() ) {
		MissingAnimClass *missing = (MissingAnimClass *)it.Get_Current();
		delete missing;
	}

	// Then clear the table
	MissingAnimTable->Reset();
}


/*
** Iterator converter from HashableClass to HAnimClass
*/
HAnimClass * HAnimManagerIterator::Get_Current_Anim( void )	
{ 
	return (HAnimClass *)Get_Current(); 
}

