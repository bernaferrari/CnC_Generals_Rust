#if defined(_MSC_VER)
#pragma once
#endif

#ifndef __AABTREESOUNDCULLCLASS_H
#define __AABTREESOUNDCULLCLASS_H

#include "AABTreeCull.H"


/////////////////////////////////////////////////////////////////////////////////
//
//	AABTreeSoundCullClass
//
//	Simple derived class that implements 2 required methods from AABTreeCullClass.
//
class AABTreeSoundCullClass : public AABTreeCullClass
{
	public:

		//////////////////////////////////////////////////////////////////////
		//	Public constructors/destructors
		//////////////////////////////////////////////////////////////////////
		AABTreeSoundCullClass (void)
			:	AABTreeCullClass (NULL)		{ }

		virtual ~AABTreeSoundCullClass (void)	{ }

		//////////////////////////////////////////////////////////////////////
		//	Public methods
		//////////////////////////////////////////////////////////////////////
		void				Load (ChunkLoadClass & cload)	{ }
		void				Save (ChunkSaveClass & csave)	{ }

	protected:

		//////////////////////////////////////////////////////////////////////
		//	Protected methods
		//////////////////////////////////////////////////////////////////////
		virtual void	Load_Node_Contents (AABTreeNodeClass * node,ChunkLoadClass & cload)	{ };
		virtual void	Save_Node_Contents (AABTreeNodeClass * node,ChunkSaveClass & csave)	{ };

};

#endif //__AABTREESOUNDCULLCLASS_H

