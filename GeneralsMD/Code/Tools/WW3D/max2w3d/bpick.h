#ifndef BPICK_H
#define BPICK_H

#include "Max.h"
//#include "dllmain.h"
//#include "resource.h"


/*
**	To use the Bone picking class, you should inherit from this class
** and implement the User_Picked... functions.  
*/
class BonePickerUserClass
{
public:
	virtual void User_Picked_Bone(INode * node) = 0;
	virtual void User_Picked_Bones(INodeTab & nodetab) = 0;
};


/*
** BonePickerClass
**	Uses Max's interface to let the user pick bones out of the scene
** or by using a dialog box to pick by name.
*/
class BonePickerClass : public PickNodeCallback, public PickModeCallback, public HitByNameDlgCallback
{
public:
	
	BonePickerClass(void) : User(NULL), BoneList(NULL), SinglePick(FALSE) {}

	/*
	** Tell this class who is using it and optionally the list
	** of bones to allow the user to select from.
	** Call this before giving this class to MAX...
	*/
	void Set_User(BonePickerUserClass * user,int singlepick = FALSE, INodeTab * bonelist = NULL) { User = user; SinglePick = singlepick; BoneList = bonelist; }

	/*
	** From BonePickNodeCallback:
	*/
	BOOL Filter(INode *node);

	/*
	** From BonePickModeCallback:
	*/
	BOOL HitTest(IObjParam *ip,HWND hWnd,ViewExp *vpt,IPoint2 m,int flags);
	BOOL Pick(IObjParam *ip,ViewExp *vpt);
		
	void EnterMode(IObjParam *ip) { }
	void ExitMode(IObjParam *ip) { }

	PickNodeCallback * GetFilter() {return this;}
	BOOL RightClick(IObjParam *ip,ViewExp *vpt) { return TRUE; }
	
	/*
	** From HitByNameDlgCallback
	*/
	virtual TCHAR * dialogTitle(void);
	virtual TCHAR * buttonText(void);
	virtual BOOL singleSelect(void) { return SinglePick; }
	virtual BOOL useFilter(void) { return TRUE; }
	virtual BOOL useProc(void) { return TRUE; }
	virtual BOOL doCustomHilite(void) { return FALSE; }
	virtual BOOL filter(INode * inode);
	virtual void proc(INodeTab & nodeTab);

protected:

	/*
	** The bone picker will pass the bones on to the "user" of
	** the class.  
	*/
	BonePickerUserClass * User;

	/*
	** List of bones that the user is being allowed to pick from.
	** If this is NULL, then the user can pick any bone
	*/
	INodeTab * BoneList;

	/*
	** Flag for whether to allow multiple selection or not
	*/
	int SinglePick;
};

extern BonePickerClass TheBonePicker;


#endif
