/*****************************************************************************
**            Includes                                                      **
*****************************************************************************/

#include <wpaudio/altypes.h>
#include <wpaudio/list.h>

// 'assignment within condition expression'.
#pragma warning(disable : 4706)

/*****************************************************************************
**          Externals                                                       **
*****************************************************************************/



/*****************************************************************************
**           Defines                                                        **
*****************************************************************************/



/*****************************************************************************
**        Private Types                                                     **
*****************************************************************************/



/*****************************************************************************
**         Private Data                                                     **
*****************************************************************************/



/*****************************************************************************
**         Public Data                                                      **
*****************************************************************************/



/*****************************************************************************
**         Private Prototypes                                               **
*****************************************************************************/



/*****************************************************************************
**          Private Functions                                               **
*****************************************************************************/

/*****************************************************************************
**          Public Functions                                                **
*****************************************************************************/

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

void		ListInit ( ListHead *head )
{
	head->prev = head->next = head;
	head->pri = (Priority) head;		// this identifies the node as a head node

}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

void		ListNodeInit ( ListNode *node )
{
	node->prev = node->next= node;
	node->pri = 0;
}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

int		ListAddNodeSortAscending( ListHead *head, ListNode *new_node )
{
	ListNode	*node;
	Priority	pri;
	int index;
	
	index = 0;
	pri = new_node->pri;
	node = (ListNode*) head;
	while( (node = ListNodeNext ( node )))
	{
		if ( pri <= node->pri )
		{
			ListNodeInsert ( node, new_node );
			return index;
		}
		index++;
	}

	ListNodeInsert ( head, new_node );
	return index;
}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

void	ListAddNode( ListHead *head, ListNode *new_node )
{
	ListNode	*node;
	Priority	pri;


	pri = new_node->pri;
	node = (ListNode*) head;
	while( (node = ListNodeNext ( node )))
	{
		if (node->pri <= pri )
		{
			ListNodeInsert ( node, new_node );
			return;
		}
	}

	ListNodeInsert ( head, new_node );
}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

void	ListAddNodeAfter( ListHead *head, ListNode *new_node )
{
	ListNode	*node;
	Priority	pri;


	pri = new_node->pri;
	node = (ListNode*) head;
	while( (node = ListNodeNext ( node )))
	{
		if (node->pri < pri )
		{
			ListNodeInsert ( node, new_node );
			return;
		}
	}

	ListNodeInsert ( head, new_node );
}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

void		ListMerge( ListHead *from, ListHead *to )
{
	ListNode	*first,
						*last,
						*node;

	first = from->next;
	last = from->prev;
	
	if ( first == (ListNode*) from )
	{
		/* the from list is empty so there is nothing to do */
	   	return;
	}
	
	node = to->prev;
	node->next = first;
	first->prev = node;
	last->next = (ListNode*) to;
	to->prev = last;
	
	ListInit ( from );	/* make the from list empty now */
}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

int	  		 	ListCountItems ( ListHead *head )
{
	ListNode *node;
	int	count = 0;

	node = head->next;

	while(node!=(ListNode*)head)
	{
		count++;
		node = node->next;
	}

	return count;
}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

ListNode*	ListFirstItem ( ListHead *head )
{
	return ListNextItem ((ListNode*) head );
}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

ListNode*	ListNextItem ( ListNode *node )
{
	if ( !node )
	{
		return NULL;
	}
	return ( ListNodeNext ( node ));
}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

ListNode*	ListGetItem( ListHead *head, int number )
{
	ListNode *node;

	node = head->next;

	while( node != (ListNode*) head )
	{
		if ( number-- == 0 )
		{
			return node;
		}
		node = node->next;
	}

	return NULL;
}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

void		 	ListNodeInsert( ListNode *node, ListNode *new_node )
{
	new_node->prev = node->prev;
	new_node->next = node;
	node->prev = new_node;
	new_node->prev->next = new_node;
}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

void 		 	ListNodeAppend( ListNode *node, ListNode *new_node )
{
	new_node->prev = node;
	new_node->next = node->next;
	node->next = new_node;
	new_node->next->prev = new_node;
}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

void 		 	ListNodeRemove( ListNode *node )
{
	node->prev->next = node->next;
	node->next->prev = node->prev;
	node->prev = node->next = node;		// so we know that the node is not in a list
}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

ListNode*		ListNodeNext( ListNode *node )
{
	ListNode	*next;

	next = node->next;

	if ( next && ListNodeIsHead ( next ))
	{
		return NULL;
	}

	return next;
}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

ListNode*		ListNodePrev (ListNode *node)
{
	ListNode	*next;

	next = node->prev;

	if ( ListNodeIsHead ( next ))
	{
		return NULL;
	}

	return next;
}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

ListNode*		ListNodeLoopNext (ListNode *node)
{
	ListNode	*next;

	next = node->next;

	if ( ListNodeIsHead ( next ))
	{
		// skip head node
		next = next->next;
		if ( ListNodeIsHead ( next ))
		{
			return NULL;	// it is an empty list
		}
	}

	return next;
}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

ListNode*		ListNodeLoopPrev (ListNode *node)
{
	ListNode	*next;

	next = node->prev;

	if ( ListNodeIsHead ( next ))
	{
		// skip head node
		next = next->prev;
		if ( ListNodeIsHead ( next ))
		{
			return NULL;	// it is an empty list
		}
	}

	return next;
}


