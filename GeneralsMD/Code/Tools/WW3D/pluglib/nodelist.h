#ifndef NODELIST_H
#define NODELIST_H

#include "always.h"
#include <Max.h>

#ifndef NODEFILT_H
#include "nodefilt.h"
#endif


class INodeListEntryClass;
class INodeCompareClass;


/*******************************************************************************
*	INodeListClass
*
*	This is a class that can enumerate a 3dsMax scene and build a list of
*	all of the INodes that meet your desired criteria.
*
*******************************************************************************/
class INodeListClass : public ITreeEnumProc
{
public:

	INodeListClass(TimeValue time,INodeFilterClass * nodefilter = NULL);
	INodeListClass(IScene * scene,TimeValue time,INodeFilterClass * nodefilter = NULL);
	INodeListClass(INode * root,TimeValue time,INodeFilterClass * nodefilter = NULL);
	INodeListClass(INodeListClass & copyfrom,TimeValue time,INodeFilterClass * inodefilter = NULL);
	~INodeListClass();

	void			Set_Filter(INodeFilterClass * inodefilter) { INodeFilter = inodefilter; }
	void			Insert(INodeListClass & insertlist);
	void			Insert(INode * node);
	void			Remove(int i);
	unsigned		Num_Nodes(void) const { return NumNodes; }
	INode *		operator[] (int index) const;
	void			Sort(const INodeCompareClass & node_compare);
	void			Add_Tree(INode * root);

private:

	unsigned						NumNodes;
	TimeValue					Time;
	INodeListEntryClass *	ListHead;
	INodeFilterClass *		INodeFilter;

	INodeListEntryClass * get_nth_item(int index);
	int callback(INode * node);
};


class INodeCompareClass
{
public:
	// returns <0 if nodea < node b.
	// returns =0 if nodea = node b.
	// returns >0 if nodea > node b.
	virtual int operator() (INode * nodea,INode * nodeb) const = 0;
};


#endif /*NODELIST_H*/