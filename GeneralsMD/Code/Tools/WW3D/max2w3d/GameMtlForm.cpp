#include "GameMtlForm.h"
#include "GameMtl.h"


/***********************************************************************************************
 * GameMtlFormClass::GameMtlFormClass -- constructor                                           *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   11/23/98   GTH : Created.                                                                 *
 *=============================================================================================*/
GameMtlFormClass::GameMtlFormClass
(
	IMtlParams *	imtl_params, 
	GameMtl *		mtl,
	int				pass
)
{
	IParams = imtl_params;
	TheMtl = mtl;
	PassIndex = pass;
}


/***********************************************************************************************
 * GameMtlFormClass::SetThing -- Set the material being edited by this form                    *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   11/23/98   GTH : Created.                                                                 *
 *=============================================================================================*/
void GameMtlFormClass::SetThing(ReferenceTarget * target)
{
	assert (target->SuperClassID()==MATERIAL_CLASS_ID);
	assert (target->ClassID()==GameMaterialClassID);

	TheMtl = (GameMtl *)target;
}


/***********************************************************************************************
 * GameMtlFormClass::GetThing -- get the material being edited by this form                    *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   11/23/98   GTH : Created.                                                                 *
 *=============================================================================================*/
ReferenceTarget * GameMtlFormClass::GetThing(void) 
{ 
	return (ReferenceTarget*)TheMtl; 
}


/***********************************************************************************************
 * GameMtlFormClass::DeleteThis -- delete myself                                               *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   11/23/98   GTH : Created.                                                                 *
 *=============================================================================================*/
void GameMtlFormClass::DeleteThis(void)
{
	delete this;
}


/***********************************************************************************************
 * GameMtlFormClass::ClassID -- returns the classID of the object being edited                 *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   11/23/98   GTH : Created.                                                                 *
 *=============================================================================================*/
Class_ID	GameMtlFormClass::ClassID()
{
	return GameMaterialClassID;  
}


/***********************************************************************************************
 * GameMtlFormClass::SetTime -- set the current time                                           *
 *                                                                                             *
 * INPUT:                                                                                      *
 *                                                                                             *
 * OUTPUT:                                                                                     *
 *                                                                                             *
 * WARNINGS:                                                                                   *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   11/23/98   GTH : Created.                                                                 *
 *=============================================================================================*/
void GameMtlFormClass::SetTime(TimeValue t)
{
	// child dialog classes don't have to support
	// the SetTime function.
}
