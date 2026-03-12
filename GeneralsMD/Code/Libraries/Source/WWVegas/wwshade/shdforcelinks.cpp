#include "shdforcelinks.h"
#include "wwhack.h"


/////////////////////////////////////////////////////////////////////////////
//
//	SHD_Force_Links
//
/////////////////////////////////////////////////////////////////////////////
void SHD_Force_Links ()
{
	//
	//	Force-link those modules that would be "linked-out" of the static lib
	//	because they are not directly referenced.
	//
	FORCE_LINK (SimpleShader);
	FORCE_LINK (GlossMaskShader);
	FORCE_LINK (BumpSpecShader);
	FORCE_LINK (BumpDiffShader);
	FORCE_LINK (CubeMapShader);
	FORCE_LINK (LegacyW3DShader);

	return ;
}
