import { Component, EventEmitter, Input, Output } from '@angular/core';
import { CommonModule } from '@angular/common';
import { FormsModule } from '@angular/forms';
import { ProtoAgentSpace, SpaceType } from '../../models/interface.space';

@Component({
  selector: 'app-create-space-form',
  standalone: true,
  imports: [CommonModule, FormsModule],
  templateUrl: './createspace.component.html',
})
export class CreateSpace {
  @Input() showForm: boolean = false;
  @Input() derivationOrigin: 'content' | 'meta' = 'content';
  @Input() newSpace: ProtoAgentSpace = {
    name: '',
    space_type: SpaceType.Content,
    description: '',
    origin_holon_id: ''
  };
  @Input() metadataJson: string = '';

  @Output() submit = new EventEmitter<{ space: ProtoAgentSpace; metadata: string }>();
  @Output() cancel = new EventEmitter<void>();

  onSubmit() {
    this.submit.emit({
      space: this.newSpace,
      metadata: this.metadataJson
    });
  }

  onCancel() {
    this.cancel.emit();
  }
}