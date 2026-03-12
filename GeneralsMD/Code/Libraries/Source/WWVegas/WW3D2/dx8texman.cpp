// This class manages textures that are in the default pool
// ensuring that they are released on device loss
// and created on device reset

// Note: It does NOT addref to textures because it is called in the texture
// destructor

#include "dx8texman.h"

TextureTrackerList DX8TextureManagerClass::Managed_Textures;


/***********************************************************************************************
 * DX8TextureManagerClass::Shutdown -- Shuts down the texture manager                          *
 *                                                                                             *
 *                                                                                             *
 *                                                                                             *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   4/25/2001  hy : Created.                                                                  *
 *   5/16/2002  km : Added depth stencil texture tracking and abstraction                      *
 *=============================================================================================*/
void DX8TextureManagerClass::Shutdown()
{
	while (!Managed_Textures.Is_Empty())
	{
		TextureTrackerClass *track=Managed_Textures.Remove_Head();
		delete track;
		track=NULL;
	}
}

/***********************************************************************************************
 * DX8TextureManagerClass::Add -- Adds a texture to be managed                                 *
 *                                                                                             *
 *                                                                                             *
 *                                                                                             *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   4/25/2001  hy : Created.                                                                  *
 *   5/16/2002  km : Added depth stencil texture tracking and abstraction                      *
 *=============================================================================================*/
void DX8TextureManagerClass::Add(TextureTrackerClass *track)
{
	// this function should only be called by the texture constructor
	Managed_Textures.Add(track);
}


/***********************************************************************************************
 * DX8TextureManagerClass::Remove -- Removes a texture from being managed                      *
 *                                                                                             *
 *                                                                                             *
 *                                                                                             *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   4/25/2001  hy : Created.                                                                  *
 *   5/16/2002  km : Added depth stencil texture tracking and abstraction                      *
 *=============================================================================================*/
void DX8TextureManagerClass::Remove(TextureBaseClass *tex)
{
	// this function should only be called by the texture destructor
	TextureTrackerListIterator it(&Managed_Textures);

	while (!it.Is_Done())
	{
		TextureTrackerClass *track=it.Peek_Obj();		
		if (track->Get_Texture()==tex)
		{			
			it.Remove_Current_Object();
			delete track;
			break;
		}
		it.Next();
	}
}


/***********************************************************************************************
 * DX8TextureManagerClass::Release_Textures -- Releases the internal d3d texture               *
 *                                                                                             *
 *                                                                                             *
 *                                                                                             *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   4/25/2001  hy : Created.                                                                  *
 *   5/16/2002  km : Added depth stencil texture tracking and abstraction                      *
 *=============================================================================================*/
void DX8TextureManagerClass::Release_Textures()
{
	TextureTrackerListIterator it(&Managed_Textures);

	while (!it.Is_Done())
	{
		TextureTrackerClass *track=it.Peek_Obj();		
		track->Release();
		it.Next();
	}
}


/***********************************************************************************************
 * DX8TextureManagerClass::Recreate_Textures -- Reallocates lost textures                      *
 *                                                                                             *
 *                                                                                             *
 *                                                                                             *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   4/25/2001  hy : Created.                                                                  *
 *   5/16/2002  km : Added depth stencil texture tracking and abstraction                      *
 *=============================================================================================*/
void DX8TextureManagerClass::Recreate_Textures()
{
	TextureTrackerListIterator it(&Managed_Textures);

	while (!it.Is_Done())
	{
		TextureTrackerClass *track=it.Peek_Obj();
		track->Recreate();
		track->Get_Texture()->Set_Dirty();
		it.Next();
	}
}

