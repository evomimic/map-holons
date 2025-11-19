import { Routes } from '@angular/router';
import { ContentSpace } from './components/content-spaces/content-spaces';
import { ContentSpaceDetail } from './components/content-space-detail/content-space-detail';
import { AllSpaces } from './components/all-spaces/all-spaces';
import { Settings } from './components/settings/settings';


export const routes: Routes = [
    { path: '', component: AllSpaces},
    {
        path: 'contentspaces',
        // This parent route provides the static "Content Spaces" breadcrumb
        data: { breadcrumb: 'Content Spaces' },
        children: [
        {
            // The list view at /contentspaces
            path: '',
            component: ContentSpace,
            // This route has no extra breadcrumb data, so the trail will end here
        },
        {
            // The detail view at /contentspaces/:id
            path: ':id',
            component: ContentSpaceDetail,
            data: { breadcrumb: 'Content Space Detail' }
        }
        ]
    },
       {
        path: 'settings',
        component: Settings,
        data: { breadcrumb: 'Settings' }
    }
    /*{
        path: 'metaspaces',
        // This parent route provides the static "Meta Spaces" breadcrumb
        data: { breadcrumb: 'Meta Spaces' },
        children: [
        {
            // The list view at /metaspaces
            path: '',
            component: MetaSpace,
            // This route has no extra breadcrumb data, so the trail will end here
        },
        {
            // The detail view at /metaspaces/:id
            path: ':id',
            component: MetaSpaceDetail,
            data: { breadcrumb: 'Meta Space Detail' }
        }
        ]
    },*/
];
