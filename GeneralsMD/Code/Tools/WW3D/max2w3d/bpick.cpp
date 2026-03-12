#include "bpick.h"
#include "dllmain.h"
#include "resource.h"


/*
** Global instance of a bone picker :-)
*/ 
BonePickerClass TheBonePicker;


/*********************************************************************************************** 
 * BonePickerClass::Filter -- determine whether the passed node is suitable                    * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   10/26/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
BOOL BonePickerClass::Filter(INode *node)
{
	if (BoneList == NULL) {
		ObjectState os  = node->EvalWorldState(0);
		if (os.obj) {
			return TRUE;
		}

	} else {
		for (int i=0; i<BoneList->Count(); i++) {
			if ((*BoneList)[i] == node) return TRUE;
		}
	}

	return FALSE;
}

/*********************************************************************************************** 
 * BonePickerClass::HitTest -- MAX HitTest method                                              * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   10/26/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
BOOL BonePickerClass::HitTest(IObjParam *ip,HWND hwnd,ViewExp *vpt,IPoint2 m,int flags)
{
	if (ip->PickNode(hwnd,m,GetFilter())) {
		return TRUE;
	} else {
		return FALSE;
	}
}

/*********************************************************************************************** 
 * BonePickerClass::Pick -- MAX Pick method                                                    * 
 *                                                                                             * 
 * INPUT:                                                                                      * 
 *                                                                                             * 
 * OUTPUT:                                                                                     * 
 *                                                                                             * 
 * WARNINGS:                                                                                   * 
 *                                                                                             * 
 * HISTORY:                                                                                    * 
 *   10/26/1997 GH  : Created.                                                                 * 
 *=============================================================================================*/
BOOL BonePickerClass::Pick(IObjParam *ip,ViewExp *vpt)
{
	INode *node = vpt->GetClosestHit();
	
	if (node) {

		/*
		** Tell the "owning" skin modifier about the
		** bone which was picked.
		*/
		assert(User);
		User->User_Picked_Bone(node);
		User = NULL;
		BoneList = NULL;
	}

	return TRUE;
}

BOOL BonePickerClass::filter(INode * inode)
{
	return Filter(inode);
}

void BonePickerClass::proc(INodeTab & nodetab)
{
	assert(User != NULL);
	User->User_Picked_Bones(nodetab);	
	User = NULL;
	BoneList = NULL;
}

TCHAR * BonePickerClass::dialogTitle(void) 
{ 
	return Get_String(IDS_PICK_BONE_DIALOG_TITLE); 
}

TCHAR * BonePickerClass::buttonText(void) 
{ 
	return Get_String(IDS_PICK_BONE_BUTTON_TEXT); 
}
