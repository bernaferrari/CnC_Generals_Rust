#if defined(_MSC_VER)
#pragma once
#endif


#ifndef WW3DIDS_H
#define WW3DIDS_H

#include "saveloadids.h"

/*
** These are the chunk-id's used by all persistant objects in WW3D.  The persistant object
** framework is defined in the WWSaveLoad library.
**
** Sept 23, 1999
** - Initial implementation of making the Commando engine persistant included making some
**   of WW3D persistant.  For this initial implementation, we're going to assume that we
**   can re-create all of our game objects from the asset manager and patch up any state
**   changes with custom game object code.  Therefore, the base class RenderObjClass has
**   a persist manager which simply saves the name of the render object and its transform
**   and re-creates that render object from the asset manager.
**
** - Currently lights are procedurally created rather than created from the asset manager.
**   For this reason, I'm implementing a persist manager and save-load functionality for
**   light class.  In the future lights may come from the asset manager in which case we
**   could remove this code.
**
** - It is also possible that later on when we make the save game stuff more robust we may
**   need to implement actual save load calls for more render objects but hopefully we can
**   avoid that since it will at least double the size of our files and just seems like a
**   lot of work to solve a few specific problems.
*/
enum
{
	WW3D_PERSIST_CHUNKID_RENDEROBJ		= CHUNKID_WW3D_BEGIN,
	WW3D_PERSIST_CHUNKID_LIGHT,
	WW3D_PERSIST_CHUNKID_DAZZLE,
};

#endif