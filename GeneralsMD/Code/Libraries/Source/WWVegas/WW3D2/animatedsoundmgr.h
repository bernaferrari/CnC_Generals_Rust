//
// MBL Update for CNC3 INCURSION - 10.23.2002 - Expanded param handling, Added STOP command
//

#if defined(_MSC_VER)
#pragma once
#endif

#ifndef __ANIMATEDSOUNDMGR_H
#define __ANIMATEDSOUNDMGR_H

#include "simplevec.h"
#include "vector.h"
#include "hashtemplate.h"


//////////////////////////////////////////////////////////////////////
//	Forward declarations
//////////////////////////////////////////////////////////////////////
class HTreeClass;
class HAnimClass;
class Matrix3D;
class SoundLibraryBridgeClass;

//////////////////////////////////////////////////////////////////////
//
//	AnimatedSoundMgrClass
//
//////////////////////////////////////////////////////////////////////
class AnimatedSoundMgrClass
{
public:

	///////////////////////////////////////////////////////////////////
	//	Public methods
	///////////////////////////////////////////////////////////////////

	//
	//	Initialization and shutdown
	//
	static void		Initialize (const char *ini_filename = NULL);
	static void		Shutdown (void);

	//
	//	Sound playback
	//
	static const char*	Get_Embedded_Sound_Name (HAnimClass *anim);
	static float			Trigger_Sound (HAnimClass *anim, float old_frame, float new_frame, const Matrix3D &tm);

	// Bridges E&B code with WW3D.
	static void		Set_Sound_Library(SoundLibraryBridgeClass* library);
	
private:

	///////////////////////////////////////////////////////////////////
	//	Private data types
	///////////////////////////////////////////////////////////////////
	struct AnimSoundInfo
	{
		AnimSoundInfo() : Frame(0), SoundName(), Is2D(false), IsStop(false) {}
		int			Frame;
		StringClass	SoundName;
		bool			Is2D;
		bool			IsStop;
	};

	typedef AnimSoundInfo								ANIM_SOUND_INFO;

	struct AnimSoundList
	{
		AnimSoundList() : List(), BoneName("root") {}
		~AnimSoundList() 
		{
			for (int i = 0; i < List.Count(); i++) {
				delete List[i];
			}
		}
		void	Add_Sound_Info(ANIM_SOUND_INFO* info) {List.Add(info);}

		SimpleDynVecClass<ANIM_SOUND_INFO*>	List;
		StringClass									BoneName;
	};

	typedef AnimSoundList								ANIM_SOUND_LIST;
	
	///////////////////////////////////////////////////////////////////
	//	Private member data
	///////////////////////////////////////////////////////////////////
	static HashTemplateClass<StringClass, ANIM_SOUND_LIST *> AnimationNameHash;
	static DynamicVectorClass<ANIM_SOUND_LIST *>					AnimSoundLists;

	static SoundLibraryBridgeClass*									SoundLibrary;

	///////////////////////////////////////////////////////////////////
	//	Private methods
	///////////////////////////////////////////////////////////////////
	static ANIM_SOUND_LIST *	Find_Sound_List (HAnimClass *anim);
};


#endif //__ANIMATEDSOUNDMGR_H
