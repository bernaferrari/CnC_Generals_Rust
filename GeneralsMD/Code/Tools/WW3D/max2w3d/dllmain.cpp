#include <stdio.h>
#include <Max.h>
#include "dllmain.h"
#include "w3ddesc.h"
#include "w3dexp.h"
#include "w3dutil.h"
#include "skin.h"
#include "gamemtl.h"
#include "gamemaps.h"
#include "MeshDeform.H"
#include "AlphaModifier.h"
#include "gridsnapmodifier.h"

#include "resource.h"

#define DLLEXPORT __declspec(dllexport)


/*****************************************************************************
*	Globals
*****************************************************************************/

HINSTANCE					AppInstance = NULL;
static int					ControlsInit = FALSE;
static W3dClassDesc		W3d_Export_Class_Descriptor;



/*********************************************************************************************** 
 * DllMain -- Entry point for the dll                                                          * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   06/09/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
BOOL WINAPI DllMain(HINSTANCE	hinstDLL,ULONG /*fdwReason*/,LPVOID /*lpvReserved*/)
{
	AppInstance = hinstDLL;

	if ( !ControlsInit )
	{
		ControlsInit = TRUE;
		InitCustomControls(AppInstance);
		InitCommonControls();
	}

	return  TRUE;
}


/*********************************************************************************************** 
 * LibDescription -- Returns description of this library                                       * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   06/09/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
DLLEXPORT const TCHAR * LibDescription()
{
	return  Get_String(IDS_LIB_DESCRIPTION);
}

/*********************************************************************************************** 
 * LibNumberClasses -- Returns number of classes in this library                               * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   06/09/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
DLLEXPORT int LibNumberClasses()
{
	return 9; //Moumine 7/24/2001    4:38:27 PM was 10. Removed Mesh_Deformation(#6)
}


/*********************************************************************************************** 
 * LibClassDesc -- Returns a ClassDesc for the specified class                                 * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   06/09/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
DLLEXPORT ClassDesc * LibClassDesc(int i)
{
	switch(i)
	{
		case 0:  return & W3d_Export_Class_Descriptor; break;
		case 1:  return Get_W3D_Utility_Desc(); break;
		case 2:	return Get_Skin_Obj_Desc(); break;
		case 3:	return Get_Skin_Mod_Desc(); break;
		case 4:	return Get_Game_Material_Desc(); break;
		case 5:	return Get_Game_Maps_Desc(); break;
		case 6:	return Get_PS2_Game_Material_Desc(); break;
		case 7:	return Get_PS2_Material_Conversion(); break;
		case 8:	return Get_Alpha_Desc(); break;
		//case 6:	return Get_Mesh_Deform_Desc(); break;
		//Moumine 7/24/2001    4:33:52 PM Removed #6 and shifted up instead of returning NULL
		// NULL causes a crash in "File->Summary info->Plug-in ifo..."
		default: return NULL; break;
	}
}


/*********************************************************************************************** 
 * LibVersion -- Returns the version number of this library                                    * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   06/09/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
DLLEXPORT ULONG LibVersion()
{
	return VERSION_3DSMAX;
}


/*********************************************************************************************** 
 * Get_String -- Gets a string out of the resources                                            * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   06/09/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
TCHAR * Get_String( int id )
{
	static TCHAR buf[256];
	if (AppInstance)
		return LoadString(AppInstance, id, buf, sizeof(buf)) ? buf : NULL;
	return NULL;
}


